//! Qualified hidden Excel instance, controlled through a killable authenticated
//! COM worker. No user-selected active workbook, UI input or model launch path.
#[cfg(windows)]
pub use native::{ExcelProgram, ExcelSession};
#[cfg(windows)]
mod native {
    use crate::{
        app_adapter::{
            ApplicationAction, ApplicationExecutor, ApplicationReceipt, ApplicationRequest,
        },
        app_authority::{ApplicationAuthority, ApplicationControl, ApplicationLaunchGrant},
        app_privacy::ApplicationIdentity,
        app_worker::{PendingDispatch, WorkerFailure, WorkerProgram, WorkerSession},
        excel_protocol::{Action, Config, Data},
        task_program::{Failure, FailureClass},
        workbook,
    };
    use custodian_winsec::{app_instance::WindowLease, excel::Snapshot};
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::windows::fs::OpenOptionsExt,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    fn failure(detail: &str) -> Failure {
        Failure::new(FailureClass::Unrecoverable, detail)
    }
    fn stale() -> Failure {
        Failure::new(
            FailureClass::StaleWorld,
            "Bound Excel document changed; preserve recovery state",
        )
    }
    fn read(path: &Path, office_open: bool) -> Result<(File, Vec<u8>), Failure> {
        // Excel retains a writable workbook handle after SaveAs. During that
        // session allow write sharing, then repeat with a strict read pin after
        // normal close. Never allow delete sharing or accept a partial parse.
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(if office_open { 3 } else { 1 })
            .open(path)
            .map_err(|_| failure("Could not independently reopen the owned workbook"))?;
        let metadata = file.metadata().map_err(|_| stale())?;
        if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
            return Err(stale());
        }
        let mut bytes = vec![];
        Read::by_ref(&mut file)
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| stale())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(stale());
        }
        Ok((file, bytes))
    }
    fn controlled(control: &ApplicationControl<'_>) -> Result<(), WorkerFailure> {
        control.check().map_err(|e| {
            if e.class == FailureClass::MissingAuthority {
                WorkerFailure::AuthorityChanged
            } else if control.deadline_elapsed() {
                WorkerFailure::Timeout
            } else {
                WorkerFailure::Stopped
            }
        })
    }
    pub struct ExcelProgram {
        worker: WorkerProgram,
        executable: String,
        _image: File,
    }
    impl ExcelProgram {
        pub fn verify_packaged(
            worker: &Path,
            expected_sha256: &str,
            excel: &Path,
        ) -> Result<Self, Failure> {
            use sha2::{Digest, Sha256};
            if expected_sha256.len() != 64
                || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(failure("Packaged application worker pin is invalid"));
            }
            let mut image = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(worker)
                .map_err(|_| stale())?;
            if image.metadata().map_err(|_| stale())?.len() > 64 * 1024 * 1024 {
                return Err(stale());
            }
            let mut sha = Sha256::new();
            let mut blake = blake3::Hasher::new();
            let mut buffer = [0; 16384];
            loop {
                let n = image.read(&mut buffer).map_err(|_| stale())?;
                if n == 0 {
                    break;
                }
                sha.update(&buffer[..n]);
                blake.update(&buffer[..n]);
            }
            if !format!("{:x}", sha.finalize()).eq_ignore_ascii_case(expected_sha256) {
                return Err(failure("Packaged application worker integrity mismatch"));
            }
            Self::verify(worker, blake.finalize().to_hex().as_str(), excel)
        }
        pub fn verify(worker: &Path, worker_blake3: &str, excel: &Path) -> Result<Self, Failure> {
            if !excel.is_absolute()
                || !custodian_winsec::excel::version_is_supported(
                    custodian_winsec::app_instance::file_version(excel).map_err(|_| stale())?,
                )
            {
                return Err(failure(
                    "Installed Excel version is outside the qualified adapter",
                ));
            }
            let canonical = excel.canonicalize().map_err(|_| stale())?;
            let image = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&canonical)
                .map_err(|_| stale())?;
            let executable = canonical
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string();
            Ok(Self {
                worker: WorkerProgram::verify(worker, worker_blake3)?,
                executable,
                _image: image,
            })
        }
        pub fn executable(&self) -> &str {
            &self.executable
        }
    }
    pub struct ExcelSession<'a> {
        worker: WorkerSession<'a>,
        identity: ApplicationIdentity,
        document_id: String,
        sheet_id: String,
        window: WindowLease,
        source: Snapshot,
        snapshot: Option<Snapshot>,
        output: PathBuf,
        saved: bool,
        verified: bool,
        closed: bool,
        checkpoint: &'a mut dyn PendingDispatch,
        _source_pin: File,
        _directory: custodian_winsec::guarded_copy::DirectoryGuard,
    }
    impl<'a> ExcelSession<'a> {
        #[allow(clippy::too_many_arguments)]
        pub fn launch(
            program: &'a ExcelProgram,
            directory: &Path,
            authority: &ApplicationAuthority,
            grant: &ApplicationLaunchGrant,
            cancel: &AtomicBool,
            document_id: &str,
            checkpoint: &'a mut dyn PendingDispatch,
            spawned: &mut dyn FnMut(u32, u64) -> Result<(), Failure>,
        ) -> Result<Self, Failure> {
            authority.check_launch(grant, program.executable())?;
            authority.admit_document_source(program.executable(), document_id)?;
            let directory_pin = custodian_winsec::guarded_copy::DirectoryGuard::open(directory)
                .map_err(|_| stale())?;
            let session = uuid::Uuid::new_v4();
            let source_path = directory.join("source.xlsx");
            let output = directory.join("output.xlsx");
            let (pin, bytes) = read(&source_path, false)?;
            let source = workbook::inspect(&bytes).map_err(failure)?;
            if workbook::recalculate(&source).map_err(failure)? != source {
                return Err(failure(
                    "Source workbook has stale or unsupported calculated values",
                ));
            }
            let existing_pids =
                custodian_winsec::app_instance::process_ids().map_err(|_| stale())?;
            let check = || {
                if cancel.load(Ordering::Acquire) {
                    return Err(WorkerFailure::Stopped);
                }
                authority
                    .check_launch(grant, program.executable())
                    .map_err(|_| WorkerFailure::AuthorityChanged)
            };
            let (worker, hello) = program.worker.start_excel(
                Config {
                    session,
                    source: source_path,
                    output: output.clone(),
                    existing_pids,
                },
                Duration::from_secs(60),
                check,
            )?;
            let Data::Started {
                pid,
                birth,
                window: handle,
            } = hello
            else {
                return Err(stale());
            };
            spawned(pid, birth)?;
            let measured = custodian_winsec::apps::process_identity(pid).ok_or_else(stale)?;
            if measured.creation_time != birth
                || !measured
                    .executable
                    .eq_ignore_ascii_case(program.executable())
            {
                return Err(stale());
            }
            let window = WindowLease::admit_handle(pid, birth, handle, &session.to_string())
                .map_err(|_| stale())?;
            let identity = ApplicationIdentity {
                executable: measured.executable,
                pid,
                process_generation: format!("{pid}:{birth}"),
                window: handle,
                window_generation: window.token().into(),
                privacy_revision: grant.revision(),
            };
            authority.admit_native_target(&identity, document_id)?;
            Ok(Self {
                worker,
                identity,
                document_id: document_id.into(),
                sheet_id: format!("{session}:sheet:1"),
                window,
                source,
                snapshot: None,
                output,
                saved: true,
                verified: false,
                closed: false,
                checkpoint,
                _source_pin: pin,
                _directory: directory_pin,
            })
        }
        pub fn sheet_id(&self) -> &str {
            &self.sheet_id
        }
        fn version(&self) -> String {
            self.snapshot
                .as_ref()
                .map(|s| {
                    format!(
                        "{}:{}",
                        blake3::hash(&serde_json::to_vec(s).expect("typed snapshot serialization")),
                        self.saved
                    )
                })
                .unwrap_or_else(|| "unopened".into())
        }
        pub fn request(&self, action: ApplicationAction) -> ApplicationRequest {
            ApplicationRequest {
                identity: self.identity.clone(),
                document_id: self.document_id.clone(),
                document_name: "Task-owned workbook".into(),
                document_version: self.version(),
                timeout_ms: if matches!(action, ApplicationAction::CloseDocumentNormally) {
                    30000
                } else {
                    15000
                },
                action,
            }
        }
        fn call(
            &mut self,
            action: Action,
            control: &ApplicationControl<'_>,
        ) -> Result<Data, Failure> {
            if !self.window.current() || self.closed {
                return Err(stale());
            }
            let result = self
                .worker
                .call(action, control.remaining()?, || controlled(control))?;
            Ok(result)
        }
        fn verify_output(&self) -> Result<(), Failure> {
            let (_, bytes) = read(&self.output, !self.closed)?;
            let actual = workbook::inspect(&bytes).map_err(failure)?;
            if self.snapshot.as_ref() != Some(&actual)
                || workbook::recalculate(&actual).map_err(failure)? != actual
            {
                return Err(stale());
            }
            Ok(())
        }
    }
    impl ApplicationExecutor for ExcelSession<'_> {
        fn remeasure(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            control.check()?;
            if !self.window.current() || self.closed {
                return Err(stale());
            }
            if self.snapshot.is_some() {
                match self.call(Action::Metadata, control)? {
                    Data::Metadata { window, saved }
                        if window == self.identity.window && saved == self.saved => {}
                    _ => return Err(stale()),
                }
            }
            Ok(self.request(request.action.clone()))
        }
        fn execute(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            control.check()?;
            if !matches!(
                request.action,
                ApplicationAction::InspectBoundDocument | ApplicationAction::VerifySavedOutput
            ) {
                self.checkpoint.record_pending(
                    &uuid::Uuid::new_v4().to_string(),
                    *blake3::hash(&serde_json::to_vec(request).map_err(|_| stale())?).as_bytes(),
                )?;
                control.check()?;
            }
            match &request.action {
                ApplicationAction::OpenBoundDocument if self.snapshot.is_none() => {
                    match self.call(Action::Open, control)? {
                        Data::Opened { window } if window == self.identity.window => {
                            self.snapshot = Some(self.source.clone())
                        }
                        _ => return Err(stale()),
                    }
                }
                ApplicationAction::InspectBoundDocument if self.snapshot.is_some() => {
                    match self.call(Action::Inspect, control)? {
                        Data::Snapshot { snapshot }
                            if self.snapshot.as_ref() == Some(&snapshot) => {}
                        _ => return Err(stale()),
                    }
                }
                ApplicationAction::EditBoundCells { sheet_id, cells } if !self.verified => {
                    if sheet_id != &self.sheet_id {
                        return Err(stale());
                    }
                    let expected = self.snapshot.clone().ok_or_else(stale)?;
                    let new = workbook::apply_edits(&expected, cells).map_err(failure)?;
                    match self.call(
                        Action::Edit {
                            expected,
                            sheet_id: sheet_id.clone(),
                            cells: cells.clone(),
                        },
                        control,
                    )? {
                        Data::Snapshot { snapshot } if snapshot == new => {
                            self.snapshot = Some(new);
                            self.saved = false;
                        }
                        _ => return Err(stale()),
                    }
                }
                ApplicationAction::SaveBoundDocument if !self.saved => {
                    let expected = self.snapshot.clone().ok_or_else(stale)?;
                    if !matches!(self.call(Action::Save { expected }, control)?, Data::Saved) {
                        return Err(stale());
                    }
                    self.saved = true;
                    self.verify_output()?;
                }
                ApplicationAction::VerifySavedOutput if self.saved => {
                    self.verify_output()?;
                    self.verified = true;
                }
                ApplicationAction::CloseDocumentNormally if self.verified => {
                    let expected = self.snapshot.clone().ok_or_else(stale)?;
                    if !matches!(
                        self.call(Action::Close { expected }, control)?,
                        Data::Closed
                    ) {
                        return Err(stale());
                    }
                    self.closed = true;
                    let birth = self
                        .identity
                        .process_generation
                        .split_once(':')
                        .and_then(|(_, b)| b.parse().ok())
                        .ok_or_else(stale)?;
                    let start = Instant::now();
                    while custodian_winsec::apps::process_is_running(self.identity.pid, birth)
                        .map_err(|_| stale())?
                    {
                        if start.elapsed() >= Duration::from_secs(15) {
                            return Err(failure(
                                "Excel normal exit remains unproved; recovery required",
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    self.verify_output()?;
                }
                _ => {
                    return Err(failure(
                        "Excel operation is unavailable in the current task state",
                    ))
                }
            }
            Ok(ApplicationReceipt {
                action: request.action.clone(),
                identity: request.identity.clone(),
                document_id: request.document_id.clone(),
                document_name: request.document_name.clone(),
                verified: true,
                detail: "Bound Excel operation and its exact postcondition were verified".into(),
            })
        }
    }
}
