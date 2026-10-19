//! One-shot authenticated native workers. Job/resource containment is NOT an
//! AppContainer, filesystem/network sandbox or permission to automate any app.
use crate::app_adapter::{ApplicationExecutor, ApplicationReceipt, ApplicationRequest};
use crate::app_authority::ApplicationControl;
use crate::task_program::{Failure, FailureClass};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const PROTOCOL_VERSION: u16 = 1;
pub const FRAME_LIMIT: u32 = 256 * 1024;
pub const STDERR_LIMIT: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub nonce: String,
    pub operation: Operation,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    ProbeSelf,
    ProbeOwnedWindow,
    Remeasure {
        request: Box<ApplicationRequest>,
    },
    Execute {
        request: Box<ApplicationRequest>,
    },
    StartExcel {
        config: crate::excel_protocol::Config,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub nonce: String,
    pub result: WorkerResult,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerResult {
    Probe {
        pid: u32,
        creation_time: u64,
        owned_window_checked: bool,
    },
    Measured {
        request: Box<ApplicationRequest>,
    },
    Receipt {
        receipt: Box<ApplicationReceipt>,
    },
    Unavailable,
    Excel {
        reply: crate::excel_protocol::Reply,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailure {
    InvalidConfiguration,
    Busy,
    Unavailable,
    Start,
    Transport,
    Protocol,
    Timeout,
    Stopped,
    AuthorityChanged,
    Cleanup,
}

impl From<WorkerFailure> for Failure {
    fn from(error: WorkerFailure) -> Self {
        let class = match error {
            WorkerFailure::AuthorityChanged => FailureClass::MissingAuthority,
            WorkerFailure::Unavailable => FailureClass::UnavailableCapability,
            _ => FailureClass::Unrecoverable,
        };
        Failure::new(
            class,
            format!("Application worker: {error:?}; no automatic effect replay"),
        )
    }
}

fn check_control(control: &ApplicationControl<'_>) -> Result<(), WorkerFailure> {
    control.check().map_err(|error| {
        if control.deadline_elapsed() {
            WorkerFailure::Timeout
        } else if error.class == FailureClass::MissingAuthority {
            WorkerFailure::AuthorityChanged
        } else {
            WorkerFailure::Stopped
        }
    })
}

/// Native owner must durably checkpoint before Execute dispatch. No default
/// implementation asserts durability. This is not model-facing consent.
pub trait PendingDispatch {
    fn record_pending(&mut self, nonce: &str, request_digest: [u8; 32]) -> Result<(), Failure>;
}

pub struct ApplicationWorker<'a> {
    program: &'a WorkerProgram,
    checkpoint: &'a mut dyn PendingDispatch,
}

impl<'a> ApplicationWorker<'a> {
    pub fn new(program: &'a WorkerProgram, checkpoint: &'a mut dyn PendingDispatch) -> Self {
        Self {
            program,
            checkpoint,
        }
    }
}

impl ApplicationExecutor for ApplicationWorker<'_> {
    fn remeasure(
        &mut self,
        request: &ApplicationRequest,
        control: &ApplicationControl<'_>,
    ) -> Result<ApplicationRequest, Failure> {
        control.check()?;
        let result = self.program.exchange(
            Operation::Remeasure {
                request: Box::new(request.clone()),
            },
            control.remaining()?,
            || check_control(control),
            None,
        )?;
        match result {
            WorkerResult::Measured { request } => Ok(*request),
            WorkerResult::Unavailable => Err(WorkerFailure::Unavailable.into()),
            _ => Err(WorkerFailure::Protocol.into()),
        }
    }

    fn execute(
        &mut self,
        request: &ApplicationRequest,
        control: &ApplicationControl<'_>,
    ) -> Result<ApplicationReceipt, Failure> {
        control.check()?;
        let result = self.program.exchange(
            Operation::Execute {
                request: Box::new(request.clone()),
            },
            control.remaining()?,
            || check_control(control),
            Some(self.checkpoint),
        )?;
        match result {
            WorkerResult::Receipt { receipt } => Ok(*receipt),
            WorkerResult::Unavailable => Err(WorkerFailure::Unavailable.into()),
            _ => Err(WorkerFailure::Protocol.into()),
        }
    }
}

#[cfg(windows)]
pub use native::{WorkerProgram, WorkerSession};

#[cfg(not(windows))]
pub struct WorkerProgram;

#[cfg(not(windows))]
impl WorkerProgram {
    pub fn verify(_: &Path, _: &str) -> Result<Self, WorkerFailure> {
        Err(WorkerFailure::Unavailable)
    }
    pub fn probe(
        &self,
        _: bool,
        _: &AtomicBool,
        _: Duration,
    ) -> Result<WorkerResult, WorkerFailure> {
        Err(WorkerFailure::Unavailable)
    }
    fn exchange(
        &self,
        _: Operation,
        _: Duration,
        _: impl Fn() -> Result<(), WorkerFailure>,
        _: Option<&mut dyn PendingDispatch>,
    ) -> Result<WorkerResult, WorkerFailure> {
        Err(WorkerFailure::Unavailable)
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use custodian_ipc::{read_frame_limited, write_frame};
    use custodian_winsec::appcontainer::{spawn_trusted_worker, Contained};
    use custodian_winsec::job::{Job, JobLimits};
    use custodian_winsec::worker_io::{cancel_thread_io, poll_pipe, PipePoll};
    use rand::RngCore;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::sync::{mpsc, Mutex};
    use std::time::Instant;

    pub struct WorkerProgram {
        path: PathBuf,
        // Holding this read-only, read-shared image handle prevents replacement
        // or mutation between native manifest verification and process launch.
        _image: File,
        active: Mutex<()>,
        poisoned: AtomicBool,
    }

    struct ChildGuard<'a> {
        child: Contained,
        job: Option<Job>,
        poisoned: &'a AtomicBool,
        cleanup_checked: bool,
    }
    impl Drop for ChildGuard<'_> {
        fn drop(&mut self) {
            drop(self.job.take());
            self.child.kill();
            // Startup errors also own a suspended process. Do not silently
            // leave it behind when job assignment or pre-resume checks fail.
            if !self.cleanup_checked
                && self
                    .child
                    .wait_for(Duration::from_secs(2))
                    .ok()
                    .flatten()
                    .is_none()
            {
                self.poisoned.store(true, Ordering::Release);
            }
        }
    }

    impl WorkerProgram {
        /// Path/hash must originate in native packaged configuration, never a
        /// goal/model field. This pin is not release signing or a same-user defense.
        pub fn verify(path: &Path, expected_blake3: &str) -> Result<Self, WorkerFailure> {
            let invalid = || WorkerFailure::InvalidConfiguration;
            if !path.is_absolute() || path.extension().and_then(|s| s.to_str()) != Some("exe") {
                return Err(invalid());
            }
            let path = path.canonicalize().map_err(|_| invalid())?;
            let mut image = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&path)
                .map_err(|_| invalid())?;
            let size = image.metadata().map_err(|_| invalid())?.len();
            if size == 0 || size > 64 * 1024 * 1024 {
                return Err(invalid());
            }
            let mut hash = blake3::Hasher::new();
            let mut buffer = [0; 16384];
            loop {
                let n = image.read(&mut buffer).map_err(|_| invalid())?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            if hash.finalize().to_hex().as_str() != expected_blake3 {
                return Err(invalid());
            }
            image.seek(SeekFrom::Start(0)).map_err(|_| invalid())?;
            Ok(Self {
                path,
                _image: image,
                active: Mutex::new(()),
                poisoned: AtomicBool::new(false),
            })
        }

        pub fn probe(
            &self,
            owned_window: bool,
            cancel: &AtomicBool,
            timeout: Duration,
        ) -> Result<WorkerResult, WorkerFailure> {
            self.exchange(
                if owned_window {
                    Operation::ProbeOwnedWindow
                } else {
                    Operation::ProbeSelf
                },
                timeout,
                || {
                    if cancel.load(Ordering::Acquire) {
                        Err(WorkerFailure::Stopped)
                    } else {
                        Ok(())
                    }
                },
                None,
            )
        }

        pub(super) fn exchange(
            &self,
            operation: Operation,
            timeout: Duration,
            check: impl Fn() -> Result<(), WorkerFailure>,
            mut checkpoint: Option<&mut dyn PendingDispatch>,
        ) -> Result<WorkerResult, WorkerFailure> {
            if timeout.is_zero() || timeout > Duration::from_secs(120) {
                return Err(WorkerFailure::InvalidConfiguration);
            }
            let _lease = self.active.try_lock().map_err(|_| WorkerFailure::Busy)?;
            if self.poisoned.load(Ordering::Acquire) {
                return Err(WorkerFailure::Cleanup);
            }
            check()?;
            let started = Instant::now();
            let envelope = Request {
                version: PROTOCOL_VERSION,
                nonce: uuid::Uuid::new_v4().to_string(),
                operation,
            };
            let serialized = serde_json::to_vec(&envelope).map_err(|_| WorkerFailure::Protocol)?;
            if serialized.len() > FRAME_LIMIT as usize {
                return Err(WorkerFailure::Protocol);
            }
            if let Operation::Execute { request } = &envelope.operation {
                let digest = blake3::hash(
                    &serde_json::to_vec(request).map_err(|_| WorkerFailure::Protocol)?,
                );
                checkpoint
                    .as_mut()
                    .ok_or(WorkerFailure::Unavailable)?
                    .record_pending(&envelope.nonce, *digest.as_bytes())
                    .map_err(|_| WorkerFailure::Unavailable)?;
            }
            check()?;
            let system_root = std::env::var_os("SystemRoot").ok_or(WorkerFailure::Start)?;
            if started.elapsed() >= timeout {
                return Err(WorkerFailure::Timeout);
            }
            let env = [
                ("SystemRoot", system_root.as_os_str()),
                ("windir", system_root.as_os_str()),
            ];
            let child = spawn_trusted_worker(&self.path, &env, self.path.parent())
                .map_err(|_| WorkerFailure::Start)?;
            let mut guard = ChildGuard {
                child,
                job: None,
                poisoned: &self.poisoned,
                cleanup_checked: false,
            };
            let job = Job::new(JobLimits {
                memory_mb: Some(512),
                cpu_percent: Some(25),
                below_normal_priority: true,
                process_limit: Some(1),
            })
            .map_err(|_| WorkerFailure::Start)?;
            job.assign_raw(guard.child.raw_process())
                .map_err(|_| WorkerFailure::Start)?;
            guard.job = Some(job);
            let measured = custodian_winsec::apps::process_identity(guard.child.pid)
                .ok_or(WorkerFailure::Start)?;
            check()?;
            if started.elapsed() >= timeout {
                return Err(WorkerFailure::Timeout);
            }
            guard.child.resume().map_err(|_| WorkerFailure::Start)?;
            let mut stdin = guard.child.stdin.take().ok_or(WorkerFailure::Start)?;
            let mut stdout = guard.child.stdout.take().ok_or(WorkerFailure::Start)?;
            let mut stderr = guard.child.stderr.take().ok_or(WorkerFailure::Start)?;
            let mut key = [0; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            let mut frame = Vec::new();
            write_frame(&mut frame, &envelope, &key).map_err(|_| WorkerFailure::Protocol)?;
            let (sent, written) = mpsc::sync_channel(1);
            let write_key = key;
            let writer = std::thread::spawn(move || {
                let ok = stdin
                    .write_all(&write_key)
                    .and_then(|()| stdin.write_all(&frame))
                    .is_ok();
                drop(stdin);
                let _ = sent.send(ok);
            });
            let mut bytes = Vec::new();
            let mut stderr_bytes = 0usize;
            let result = (|| {
                loop {
                    check()?;
                    if started.elapsed() >= timeout {
                        return Err(WorkerFailure::Timeout);
                    }
                    if matches!(written.try_recv(), Ok(false)) {
                        return Err(WorkerFailure::Transport);
                    }
                    if let PipePoll::Data(data) =
                        poll_pipe(&mut stdout).map_err(|_| WorkerFailure::Transport)?
                    {
                        bytes.extend_from_slice(&data);
                        if bytes.len() >= 8 {
                            if bytes[..4] != custodian_ipc::MAGIC {
                                return Err(WorkerFailure::Protocol);
                            }
                            let len = u32::from_le_bytes(
                                bytes[4..8]
                                    .try_into()
                                    .map_err(|_| WorkerFailure::Protocol)?,
                            );
                            if len > FRAME_LIMIT || bytes.len() > len as usize + 40 {
                                return Err(WorkerFailure::Protocol);
                            }
                        }
                    }
                    if let PipePoll::Data(data) =
                        poll_pipe(&mut stderr).map_err(|_| WorkerFailure::Transport)?
                    {
                        stderr_bytes += data.len();
                        if stderr_bytes > STDERR_LIMIT {
                            return Err(WorkerFailure::Protocol);
                        }
                    }
                    if let Some(code) = guard
                        .child
                        .try_wait()
                        .map_err(|_| WorkerFailure::Transport)?
                    {
                        if code != 0 {
                            return Err(WorkerFailure::Transport);
                        }
                        // Drain buffered bytes after exit, without waiting on a read.
                        while let PipePoll::Data(data) =
                            poll_pipe(&mut stdout).map_err(|_| WorkerFailure::Transport)?
                        {
                            bytes.extend_from_slice(&data);
                            if bytes.len() > FRAME_LIMIT as usize + 40 {
                                return Err(WorkerFailure::Protocol);
                            }
                        }
                        while let PipePoll::Data(data) =
                            poll_pipe(&mut stderr).map_err(|_| WorkerFailure::Transport)?
                        {
                            stderr_bytes += data.len();
                            if stderr_bytes > STDERR_LIMIT {
                                return Err(WorkerFailure::Protocol);
                            }
                        }
                        let mut input = std::io::Cursor::new(&bytes);
                        let reply: Reply = read_frame_limited(&mut input, &key, FRAME_LIMIT)
                            .map_err(|_| WorkerFailure::Protocol)?;
                        if input.position() as usize != bytes.len()
                            || reply.version != PROTOCOL_VERSION
                            || reply.nonce != envelope.nonce
                        {
                            return Err(WorkerFailure::Protocol);
                        }
                        match (&envelope.operation, &reply.result) {
                            (
                                Operation::ProbeSelf,
                                WorkerResult::Probe {
                                    pid,
                                    creation_time,
                                    owned_window_checked: false,
                                },
                            )
                            | (
                                Operation::ProbeOwnedWindow,
                                WorkerResult::Probe {
                                    pid,
                                    creation_time,
                                    owned_window_checked: true,
                                },
                            ) if *pid == measured.pid
                                && *creation_time == measured.creation_time => {}
                            (
                                Operation::Remeasure { .. },
                                WorkerResult::Measured { .. } | WorkerResult::Unavailable,
                            )
                            | (
                                Operation::Execute { .. },
                                WorkerResult::Receipt { .. } | WorkerResult::Unavailable,
                            ) => {}
                            _ => return Err(WorkerFailure::Protocol),
                        }
                        // A validated returned receipt must reach execute_checked
                        // even if consent changed during cleanup. That outer gate
                        // retains it as recovery evidence instead of losing it.
                        if !matches!(reply.result, WorkerResult::Receipt { .. }) {
                            check()?;
                        }
                        return Ok(reply.result);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })();
            // Never wait indefinitely on an unresponsive peer or pipe writer.
            drop(guard.job.take());
            guard.child.kill();
            cancel_thread_io(&writer);
            let cleanup = Instant::now();
            while !writer.is_finished() && cleanup.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(10));
            }
            let exited = guard
                .child
                .wait_for(Duration::from_secs(2))
                .ok()
                .flatten()
                .is_some();
            guard.cleanup_checked = true;
            if !writer.is_finished() || !exited {
                self.poisoned.store(true, Ordering::Release);
                return Err(WorkerFailure::Cleanup);
            }
            writer.join().map_err(|_| WorkerFailure::Cleanup)?;
            result
        }
    }
    include!("app_worker_session.rs");
}
