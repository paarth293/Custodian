#![cfg(windows)]
use custodian_core::app_adapter::{execute_checked, ApplicationAction, ApplicationRequest};
use custodian_core::app_authority::{AppPermission, ApplicationAuthority};
use custodian_core::app_privacy::{ApplicationIdentity, ApplicationPrivacyPolicy};
use custodian_core::app_worker::{
    ApplicationWorker, PendingDispatch, WorkerFailure, WorkerProgram, WorkerResult,
};
use custodian_core::task_program::{Failure, FailureClass};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

struct UnrelatedProcess(std::process::Child);
impl Drop for UnrelatedProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        for _ in 0..200 {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("unrelated fixture process cleanup did not finish");
    }
}

fn program(path: &Path) -> WorkerProgram {
    let hash = blake3::hash(&std::fs::read(path).unwrap())
        .to_hex()
        .to_string();
    WorkerProgram::verify(path, &hash).unwrap()
}

fn request() -> ApplicationRequest {
    ApplicationRequest {
        identity: ApplicationIdentity {
            executable: r"C:\Fixture\editor.exe".into(),
            pid: 123,
            process_generation: "native-process-fixture".into(),
            window: 456,
            window_generation: "native-window-fixture".into(),
            privacy_revision: 1,
        },
        document_id: "native-document-fixture".into(),
        document_name: "synthetic".into(),
        document_version: "version-1".into(),
        action: ApplicationAction::SaveBoundDocument,
        timeout_ms: 10_000,
    }
}

struct Journal {
    path: PathBuf,
    fail: bool,
}
impl PendingDispatch for Journal {
    fn record_pending(&mut self, nonce: &str, digest: [u8; 32]) -> Result<(), Failure> {
        if self.fail {
            return Err(Failure::new(
                FailureClass::Unrecoverable,
                "checkpoint unavailable",
            ));
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .unwrap();
        file.write_all(nonce.as_bytes()).unwrap();
        file.write_all(&digest).unwrap();
        file.sync_all().unwrap();
        Ok(())
    }
}

#[test]
fn actual_worker_authenticates_exits_and_probes_only_its_own_window() {
    let worker = program(Path::new(env!("CARGO_BIN_EXE_custodian-appworker")));
    for owned in [false, true] {
        match worker
            .probe(owned, &AtomicBool::new(false), Duration::from_secs(10))
            .unwrap()
        {
            WorkerResult::Probe {
                pid,
                creation_time,
                owned_window_checked,
            } => {
                assert!(pid > 0 && creation_time > 0);
                assert_eq!(owned, owned_window_checked);
                // The one-shot worker is gone when the host reports success.
                assert!(!custodian_winsec::apps::process_is_running(pid, creation_time).unwrap());
            }
            _ => panic!("unexpected worker result"),
        }
    }
}

#[test]
fn real_worker_refuses_unqualified_app_and_never_reaches_effect_checkpoint() {
    let worker = program(Path::new(env!("CARGO_BIN_EXE_custodian-appworker")));
    let req = request();
    let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 1);
    let grant = authority
        .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal {
        path: directory.path().join("must-not-exist"),
        fail: false,
    };
    let mut adapter = ApplicationWorker::new(&worker, &mut journal);
    let error = execute_checked(
        &req,
        &authority,
        &grant,
        &AtomicBool::new(false),
        &mut adapter,
    )
    .unwrap_err();
    assert_eq!(error.class, FailureClass::UnavailableCapability);
    assert!(std::fs::read_dir(directory.path())
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn invalid_pin_and_preexisting_stop_do_not_launch_a_worker() {
    let path = Path::new(env!("CARGO_BIN_EXE_custodian-appworker"));
    assert!(matches!(
        WorkerProgram::verify(path, &"0".repeat(64)),
        Err(WorkerFailure::InvalidConfiguration)
    ));
    let worker = program(path);
    assert!(matches!(
        worker.probe(false, &AtomicBool::new(true), Duration::from_secs(10)),
        Err(WorkerFailure::Stopped)
    ));
    assert!(matches!(
        worker.probe(false, &AtomicBool::new(false), Duration::ZERO),
        Err(WorkerFailure::InvalidConfiguration)
    ));
}

#[test]
#[ignore = "Requires explicitly built test-only example via CUSTODIAN_APP_WORKER_FIXTURE"]
fn actual_fault_peer_enforces_deadlines_limits_and_preserves_pending_effects() {
    let fixture = PathBuf::from(
        std::env::var_os("CUSTODIAN_APP_WORKER_FIXTURE").expect("explicit fixture path"),
    );
    let root = tempfile::tempdir().unwrap();
    for mode in [
        "hang",
        "partial",
        "oversize",
        "wrong-nonce",
        "wrong-version",
        "trailing-frame",
        "bad-mac",
        "stderr-flood",
        "reply-hang",
        "memory",
        "child-limit",
        "echo",
        "effect-ok",
        "crash-effect",
        "no-read",
        "exit-259",
    ] {
        let directory = root.path().join(mode);
        std::fs::create_dir(&directory).unwrap();
        let exe = directory.join("custodian-appworker.exe");
        std::fs::copy(&fixture, &exe).unwrap();
        let worker = program(&exe);
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        match mode {
            "echo" | "effect-ok" | "crash-effect" => {
                let req = request();
                let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 1);
                let grant = authority
                    .issue_after_native_consent(
                        &req,
                        &[AppPermission::Save],
                        Duration::from_secs(30),
                    )
                    .unwrap();
                let pending = directory.join("pending");
                let mut journal = Journal {
                    path: pending.clone(),
                    fail: mode == "echo",
                };
                let mut adapter = ApplicationWorker::new(&worker, &mut journal);
                let result = execute_checked(&req, &authority, &grant, &cancel, &mut adapter);
                if mode == "effect-ok" {
                    assert!(result.is_ok());
                } else {
                    assert!(result.is_err());
                }
                if mode == "echo" {
                    assert!(!pending.exists());
                    assert!(!directory.join("synthetic-effect.txt").exists());
                } else {
                    assert!(pending.exists());
                    assert_eq!(
                        std::fs::read(directory.join("synthetic-effect.txt")).unwrap(),
                        b"fixture effect"
                    );
                }
            }
            "no-read" => {
                let mut req = request();
                req.timeout_ms = 10_000;
                req.action = ApplicationAction::EditBoundRange {
                    start: 0,
                    end: 0,
                    replacement: "\\".repeat(64 * 1024),
                };
                let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 1);
                let grant = authority
                    .issue_after_native_consent(
                        &req,
                        &[AppPermission::Edit],
                        Duration::from_secs(30),
                    )
                    .unwrap();
                let mut journal = Journal {
                    path: directory.join("pending"),
                    fail: true,
                };
                let error = execute_checked(
                    &req,
                    &authority,
                    &grant,
                    &cancel,
                    &mut ApplicationWorker::new(&worker, &mut journal),
                )
                .unwrap_err();
                assert!(error.detail.contains("Timeout"), "{}", error.detail);
                assert!(!journal.path.exists());
            }
            _ => {
                // The deadline includes cold executable scanning/startup. Allow
                // the peer to enter its fault mode, then require its PID record
                // below so a pre-start timeout cannot count as fault coverage.
                let result = worker.probe(false, &cancel, Duration::from_secs(10));
                eprintln!("FAULT RESPONSE {mode}: {:?}", result.as_ref().err());
                if matches!(mode, "child-limit" | "memory") {
                    assert!(result.is_ok());
                } else {
                    let expected = match mode {
                        "hang" | "partial" | "reply-hang" => WorkerFailure::Timeout,
                        "exit-259" => WorkerFailure::Transport,
                        _ => WorkerFailure::Protocol,
                    };
                    assert_eq!(
                        result.err(),
                        Some(expected),
                        "wrong fault classification: {mode}"
                    );
                }
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(16),
            "unbounded fault run: {mode}"
        );
        // A second call after a normal timeout/protocol error must not encounter
        // stale pipes/frames. It still fails the same fault, never succeeds late.
        if mode == "hang" {
            use std::os::windows::process::CommandExt;
            let mut unrelated = UnrelatedProcess(
                std::process::Command::new(&exe)
                    .arg("--sentinel")
                    .creation_flags(0x08000000)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let stop = AtomicBool::new(false);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(100));
                    stop.store(true, Ordering::Release);
                });
                assert!(matches!(
                    worker.probe(false, &stop, Duration::from_secs(10)),
                    Err(WorkerFailure::Stopped)
                ));
            });
            assert!(
                unrelated.0.try_wait().unwrap().is_none(),
                "worker cleanup killed an unrelated fixture"
            );
            for policy_update in [false, true] {
                let mut req = request();
                req.timeout_ms = 10_000;
                req.identity.privacy_revision = 0;
                let authority = ApplicationAuthority::open(
                    directory.join(format!("live-policy-{policy_update}.dpapi")),
                )
                .unwrap();
                let grant = authority
                    .issue_after_native_consent(
                        &req,
                        &[AppPermission::Save],
                        Duration::from_secs(30),
                    )
                    .unwrap();
                let log = directory.join("worker-pids.txt");
                let before = std::fs::read_to_string(&log).unwrap().lines().count();
                let mut journal = Journal {
                    path: directory.join(format!("revoked-pending-{policy_update}")),
                    fail: false,
                };
                std::thread::scope(|scope| {
                    scope.spawn(|| {
                        let start = Instant::now();
                        while std::fs::read_to_string(&log).unwrap().lines().count() <= before {
                            assert!(
                                start.elapsed() < Duration::from_secs(10),
                                "revocation worker never started"
                            );
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        if policy_update {
                            authority
                                .replace_private_executables(
                                    0,
                                    vec![req.identity.executable.clone()],
                                )
                                .unwrap();
                        } else {
                            grant.revoke();
                        }
                    });
                    let error = execute_checked(
                        &req,
                        &authority,
                        &grant,
                        &cancel,
                        &mut ApplicationWorker::new(&worker, &mut journal),
                    )
                    .unwrap_err();
                    assert_eq!(error.class, FailureClass::MissingAuthority);
                });
                assert!(!journal.path.exists());
                if policy_update {
                    assert_eq!(authority.settings().unwrap().revision, 1);
                    eprintln!(
                        "PERSISTED POLICY UPDATE STOPPED A RUNNING WORKER BEFORE EFFECT DISPATCH"
                    );
                }
            }
        }
        if mode == "exit-259" {
            assert!(matches!(
                worker.probe(false, &cancel, Duration::from_secs(10)),
                Err(WorkerFailure::Transport)
            ));
        }
        let records = std::fs::read_to_string(directory.join("worker-pids.txt")).unwrap();
        for line in records.lines() {
            let mut values = line.split_whitespace();
            let pid = values.next().unwrap().parse().unwrap();
            let generation = values.next().unwrap().parse().unwrap();
            assert!(
                !custodian_winsec::apps::process_is_running(pid, generation).unwrap(),
                "worker survived cleanup: {mode}"
            );
        }
        eprintln!("WORKER FAULT VERIFIED: {mode}");
    }
}

#[test]
#[ignore = "Requires an explicitly copied optimized worker via CUSTODIAN_APP_WORKER_RELEASE"]
fn optimized_worker_operates_outside_checkout() {
    let path = PathBuf::from(
        std::env::var_os("CUSTODIAN_APP_WORKER_RELEASE").expect("explicit release path"),
    );
    let canonical = path.canonicalize().unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    assert!(
        !canonical.starts_with(checkout),
        "release qualification must run outside checkout"
    );
    let worker = program(&canonical);
    for owned in [false, true] {
        let WorkerResult::Probe {
            pid,
            creation_time,
            owned_window_checked,
        } = worker
            .probe(owned, &AtomicBool::new(false), Duration::from_secs(10))
            .unwrap()
        else {
            panic!("unexpected release response")
        };
        assert_eq!(owned, owned_window_checked);
        assert!(!custodian_winsec::apps::process_is_running(pid, creation_time).unwrap());
    }
    let req = request();
    let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 1);
    let grant = authority
        .issue_after_native_consent(&req, &[AppPermission::Save], Duration::from_secs(30))
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal {
        path: directory.path().join("must-not-exist"),
        fail: false,
    };
    let error = execute_checked(
        &req,
        &authority,
        &grant,
        &AtomicBool::new(false),
        &mut ApplicationWorker::new(&worker, &mut journal),
    )
    .unwrap_err();
    assert_eq!(error.class, FailureClass::UnavailableCapability);
    assert!(!journal.path.exists());
    eprintln!(
        "OPTIMIZED WORKER VERIFIED OUTSIDE CHECKOUT: {}",
        canonical.display()
    );
}
