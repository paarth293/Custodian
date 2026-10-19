//! Dedicated-profile editor API bridge. Native callers own consent, source
//! staging, durable checkpoints and publication. No model-facing launch path.
#[cfg(windows)]
pub use native::{EditorProgram, EditorSession};

#[cfg(windows)]
mod native {
    use crate::{
        app_adapter::{
            ApplicationAction, ApplicationExecutor, ApplicationReceipt, ApplicationRequest,
        },
        app_authority::{ApplicationAuthority, ApplicationControl, ApplicationLaunchGrant},
        app_privacy::ApplicationIdentity,
        app_worker::PendingDispatch,
        task_program::{Failure, FailureClass},
    };
    use custodian_ipc::{read_frame_limited, write_frame};
    use custodian_winsec::{
        app_instance::WindowLease,
        local_pipe::Server,
        worker_io::{poll_pipe, PipePoll},
    };
    use rand::RngCore;
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const LIMIT: u32 = 256 * 1024;
    const FILES: [(&str, &[u8]); 3] = [
        (
            "package.json",
            include_bytes!("../../../services/editor-adapter/package.json"),
        ),
        (
            "extension.cjs",
            include_bytes!("../../../services/editor-adapter/extension.cjs"),
        ),
        (
            "wire.cjs",
            include_bytes!("../../../services/editor-adapter/wire.cjs"),
        ),
    ];
    fn failure(message: &str) -> Failure {
        Failure::new(FailureClass::Unrecoverable, message)
    }
    fn stale() -> Failure {
        Failure::new(
            FailureClass::StaleWorld,
            "Bound editor document changed; inspect recovery state",
        )
    }
    fn wait_for_process_exit(
        root: (u32, u64),
        client: (u32, u64),
        check: impl Fn() -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        // The caller owns the action deadline and its Stop/authority checks.
        // An extra local timeout must not truncate the remaining Close budget.
        loop {
            check()?;
            let root_running =
                custodian_winsec::apps::process_is_running(root.0, root.1).map_err(|_| stale())?;
            let client_running = custodian_winsec::apps::process_is_running(client.0, client.1)
                .map_err(|_| stale())?;
            if !root_running && !client_running {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn text_path(path: &Path) -> String {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string()
    }
    fn sha(text: &str) -> String {
        format!("{:x}", Sha256::digest(text.as_bytes()))
    }
    fn read_text(path: &Path) -> Result<String, Failure> {
        let file = File::open(path).map_err(|_| failure("Task document unavailable"))?;
        let meta = file.metadata().map_err(|_| stale())?;
        if !meta.is_file()
            || meta.len() > 65536
            || meta.file_attributes() & (0x400 | 0x1000 | 0x400000) != 0
            || custodian_winsec::fsops::file_link_count(path).map_err(|_| stale())? != 1
        {
            return Err(stale());
        }
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| stale())?;
        if bytes.len() > 65536 {
            return Err(stale());
        }
        String::from_utf8(bytes).map_err(|_| failure("Only bounded UTF-8 text is qualified"))
    }

    pub struct EditorProgram {
        executable: PathBuf,
        extension: PathBuf,
        _pins: Vec<File>,
        _directory: custodian_winsec::guarded_copy::DirectoryGuard,
    }
    impl EditorProgram {
        /// Native packaged/configured locations only. Executable version and
        /// extension bytes must match this qualified adapter build.
        pub fn verify(executable: &Path, extension: &Path) -> Result<Self, Failure> {
            if !executable.is_absolute() || !extension.is_absolute() {
                return Err(failure("Native editor configuration required"));
            }
            let executable = executable
                .canonicalize()
                .map_err(|_| failure("Qualified editor unavailable"))?;
            if custodian_winsec::app_instance::file_version(&executable)
                .map_err(|_| failure("Editor version unavailable"))?
                != [1, 140, 0, 0]
            {
                return Err(Failure::new(
                    FailureClass::UnavailableCapability,
                    "This editor version has not been qualified",
                ));
            }
            let mut pins = vec![OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&executable)
                .map_err(|_| failure("Editor image unavailable"))?];
            let extension = extension
                .canonicalize()
                .map_err(|_| failure("Bundled editor adapter unavailable"))?;
            let directory_pin = custodian_winsec::guarded_copy::DirectoryGuard::open(&extension)
                .map_err(|_| stale())?;
            for (name, expected) in FILES {
                let mut file = OpenOptions::new()
                    .read(true)
                    .share_mode(1)
                    .open(extension.join(name))
                    .map_err(|_| failure("Bundled editor adapter unavailable"))?;
                if file.metadata().map_err(|_| stale())?.len() != expected.len() as u64 {
                    return Err(failure("Editor adapter integrity mismatch"));
                }
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).map_err(|_| stale())?;
                if bytes != expected {
                    return Err(failure("Editor adapter integrity mismatch"));
                }
                pins.push(file);
            }
            Ok(Self {
                executable,
                extension,
                _pins: pins,
                _directory: directory_pin,
            })
        }
        pub fn executable(&self) -> String {
            text_path(&self.executable)
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Response {
        session: String,
        seq: u64,
        result: ResultData,
    }
    #[derive(Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "snake_case",
        rename_all_fields = "camelCase",
        deny_unknown_fields
    )]
    enum ResultData {
        Hello {
            pid: u32,
            parent_pid: u32,
            version: String,
        },
        Bound {
            document_id: String,
            version: u64,
            dirty: bool,
        },
        Metadata {
            document_id: String,
            version: u64,
            dirty: bool,
        },
        Snapshot {
            document_id: String,
            version: u64,
            text: String,
            digest: String,
        },
        Edited {
            document_id: String,
            version: u64,
            digest: String,
        },
        Saved {
            document_id: String,
            version: u64,
            digest: String,
        },
        Closed {
            document_id: String,
        },
        WindowCloseReady,
        Refused {
            detail: String,
        },
    }
    #[derive(Serialize)]
    struct Envelope<'a> {
        session: &'a str,
        seq: u64,
        command: serde_json::Value,
    }

    pub struct EditorSession<'a> {
        _program: &'a EditorProgram,
        pipe: Server,
        key: [u8; 32],
        session: String,
        sequence: u64,
        _child: Child,
        root: (u32, u64),
        client: (u32, u64),
        window: WindowLease,
        identity: ApplicationIdentity,
        document_id: String,
        target: PathBuf,
        text: String,
        digest: String,
        version: Option<u64>,
        closed: bool,
        checkpoint: &'a mut dyn PendingDispatch,
        _directory: custodian_winsec::guarded_copy::DirectoryGuard,
    }
    impl<'a> EditorSession<'a> {
        #[allow(clippy::too_many_arguments)]
        pub fn launch(
            program: &'a EditorProgram,
            directory: &Path,
            target: &Path,
            authority: &ApplicationAuthority,
            grant: &ApplicationLaunchGrant,
            cancel: &AtomicBool,
            document_id: &str,
            checkpoint: &'a mut dyn PendingDispatch,
            spawned: &mut dyn FnMut(u32, u64) -> Result<(), Failure>,
        ) -> Result<Self, Failure> {
            authority.check_launch(grant, &program.executable())?;
            authority.admit_document_source(&program.executable(), document_id)?;
            let directory_pin = custodian_winsec::guarded_copy::DirectoryGuard::open(directory)
                .map_err(|_| stale())?;
            if !directory.is_absolute()
                || target.parent() != Some(directory)
                || target.file_name().and_then(|n| n.to_str()) != Some("document.txt")
            {
                return Err(failure("Native task-owned staging required"));
            }
            let text = read_text(target)?;
            let session = uuid::Uuid::new_v4().to_string();
            let mut pipe =
                Server::new(&session).map_err(|_| failure("Private editor IPC unavailable"))?;
            let mut key = [0; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            let profile = directory.join("editor-profile");
            let extensions = directory.join("editor-extensions");
            fs::create_dir(&profile).map_err(|_| failure("Fresh editor profile required"))?;
            fs::create_dir(&extensions)
                .map_err(|_| failure("Fresh editor extensions directory required"))?;
            fs::create_dir(profile.join("User")).map_err(|_| stale())?;
            let settings = serde_json::json!({"telemetry.telemetryLevel":"off","update.mode":"none",
                "extensions.autoCheckUpdates":false,"extensions.autoUpdate":false,"workbench.enableExperiments":false,
                "workbench.startupEditor":"none","window.restoreWindows":"none","files.hotExit":"off","files.autoSave":"off",
                "git.enabled":false,"chat.disableAIFeatures":true,"security.workspace.trust.enabled":true,
                "editor.formatOnSave":false,"editor.codeActionsOnSave":{},"files.encoding":"utf8",
                "files.insertFinalNewline":false,"files.trimFinalNewlines":false,"files.trimTrailingWhitespace":false});
            let mut config = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(profile.join("User/settings.json"))
                .map_err(|_| stale())?;
            config
                .write_all(&serde_json::to_vec(&settings).map_err(|_| stale())?)
                .map_err(|_| stale())?;
            config.sync_all().map_err(|_| stale())?;
            let descriptor = directory.join("editor-bootstrap.json");
            let expires = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| stale())?
                .as_millis()
                + 600000;
            let secret = key.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&descriptor)
                .map_err(|_| stale())?;
            file.write_all(
                &serde_json::to_vec(
                    &serde_json::json!({"schema":1,"session":session,"target":text_path(target),
                "pipe":pipe.name(),"secret":secret,"expires":expires}),
                )
                .map_err(|_| stale())?,
            )
            .map_err(|_| stale())?;
            file.sync_all().map_err(|_| stale())?;
            drop(file);
            let start = Instant::now();
            let check = || {
                if cancel.load(Ordering::Acquire) {
                    return Err(failure("Application task stopped"));
                }
                if start.elapsed() > Duration::from_secs(60) {
                    return Err(failure("Editor startup deadline elapsed"));
                }
                authority.check_launch(grant, &program.executable())
            };
            check()?;
            let mut child = Command::new(&program.executable)
                .args([
                    "--new-window",
                    "--skip-welcome",
                    "--skip-release-notes",
                    "--disable-updates",
                    "--disable-extensions",
                ])
                .arg("--user-data-dir")
                .arg(text_path(&profile))
                // VS Code 1.140 shares application storage independently of
                // --user-data-dir. Bind every mutable store to this owned task.
                .arg("--shared-data-dir")
                .arg(text_path(&directory.join("editor-shared")))
                .arg("--agents-user-data-dir")
                .arg(text_path(&directory.join("editor-agents")))
                .arg("--agents-extensions-dir")
                .arg(text_path(&extensions))
                .arg("--agent-plugins-dir")
                .arg(text_path(&extensions))
                .arg("--extensions-download-dir")
                .arg(text_path(&directory.join("editor-downloads")))
                .arg("--extensions-dir")
                .arg(text_path(&extensions))
                .arg("--extensionDevelopmentPath")
                .arg(text_path(&program.extension))
                .env_remove("ELECTRON_RUN_AS_NODE")
                .env_remove("NODE_OPTIONS")
                .env_remove("VSCODE_DEV")
                .env_remove("VSCODE_PORTABLE")
                .env("CUSTODIAN_EDITOR_DESCRIPTOR", text_path(&descriptor))
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| failure("Could not launch the qualified editor"))?;
            let native = custodian_winsec::apps::process_identity(child.id()).ok_or_else(stale)?;
            spawned(native.pid, native.creation_time)?;
            let mut decoder = Vec::new();
            let hello = loop {
                check()?;
                if child.try_wait().map_err(|_| stale())?.is_some() {
                    return Err(failure("Editor exited before admission"));
                }
                if pipe
                    .try_connect()
                    .map_err(|_| failure("Editor IPC connection failed"))?
                {
                    if let Some(reply) = receive(&mut pipe, &mut decoder, &key)? {
                        break reply;
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let ResultData::Hello {
                pid,
                parent_pid,
                version,
            } = hello.result
            else {
                return Err(failure("Invalid editor admission response"));
            };
            if hello.session != session
                || hello.seq != 0
                || version != "1.140.0"
                || pid != pipe.client_pid().map_err(|_| stale())?
                || !custodian_winsec::app_instance::is_descendant(
                    pid,
                    native.pid,
                    native.creation_time,
                )
                || parent_pid == 0
            {
                return Err(failure("Editor peer identity mismatch"));
            }
            let client = custodian_winsec::apps::process_identity(pid).ok_or_else(stale)?;
            if text_path(Path::new(&client.executable)).to_lowercase()
                != program.executable().to_lowercase()
            {
                return Err(failure("Editor peer executable mismatch"));
            }
            let window = loop {
                check()?;
                if let Ok(window) = WindowLease::admit(native.pid, native.creation_time, &session) {
                    break window;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let identity = ApplicationIdentity {
                executable: program.executable(),
                pid: native.pid,
                process_generation: format!("{}:{}", native.pid, native.creation_time),
                window: window.handle(),
                window_generation: window.token().into(),
                privacy_revision: grant.revision(),
            };
            authority.admit_native_target(&identity, document_id)?;
            let digest = sha(&text);
            Ok(Self {
                _program: program,
                pipe,
                key,
                session,
                sequence: 0,
                _child: child,
                root: (native.pid, native.creation_time),
                client: (pid, client.creation_time),
                window,
                identity,
                document_id: document_id.into(),
                target: target.into(),
                text,
                digest,
                version: None,
                closed: false,
                checkpoint,
                _directory: directory_pin,
            })
        }

        pub fn request(&self, action: ApplicationAction) -> ApplicationRequest {
            let timeout_ms = if matches!(action, ApplicationAction::CloseDocumentNormally) {
                30000
            } else {
                10000
            };
            ApplicationRequest {
                identity: self.identity.clone(),
                document_id: self.document_id.clone(),
                document_name: "Bound task-owned text document".into(),
                document_version: self
                    .version
                    .map(|v| format!("{v}:{}", self.digest))
                    .unwrap_or_else(|| "unopened".into()),
                action,
                timeout_ms,
            }
        }
        pub fn expected_text(&self) -> &str {
            &self.text
        }
        fn transact(
            &mut self,
            command: serde_json::Value,
            control: &ApplicationControl<'_>,
        ) -> Result<ResultData, Failure> {
            control.check()?;
            if !self.window.current() {
                return Err(stale());
            }
            self.sequence += 1;
            let mut frame = Vec::new();
            write_frame(
                &mut frame,
                &Envelope {
                    session: &self.session,
                    seq: self.sequence,
                    command,
                },
                &self.key,
            )
            .map_err(|_| failure("Editor request framing failed"))?;
            if frame.len() > LIMIT as usize + 40 {
                return Err(failure("Editor request limit"));
            }
            self.pipe
                .file()
                .write_all(&frame)
                .map_err(|_| failure("Editor request transport failed"))?;
            let mut buffer = Vec::new();
            loop {
                control.check()?;
                if !self.window.current() {
                    return Err(stale());
                }
                if let Some(reply) = receive(&mut self.pipe, &mut buffer, &self.key)? {
                    if reply.session != self.session || reply.seq != self.sequence {
                        return Err(failure("Editor reply sequence mismatch"));
                    }
                    if let ResultData::Refused { detail } = reply.result {
                        // Peer error text is intentionally not copied into host logs/context.
                        drop(detail);
                        return Err(stale());
                    }
                    control.check()?;
                    return Ok(reply.result);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        fn metadata(&mut self, control: &ApplicationControl<'_>) -> Result<(u64, bool), Failure> {
            let result = self.transact(serde_json::json!({"kind":"metadata"}), control)?;
            let ResultData::Metadata {
                document_id,
                version,
                dirty,
            } = result
            else {
                return Err(stale());
            };
            if document_id != self.document_id {
                return Err(stale());
            }
            Ok((version, dirty))
        }
        pub fn close_window(&mut self, control: &ApplicationControl<'_>) -> Result<(), Failure> {
            if !self.closed {
                return Err(failure(
                    "Close the bound document before closing its window",
                ));
            }
            if !matches!(
                self.transact(serde_json::json!({"kind":"close_window"}), control)?,
                ResultData::WindowCloseReady
            ) {
                return Err(stale());
            }
            wait_for_process_exit(self.root, self.client, || control.check())
        }
    }

    fn receive(
        pipe: &mut Server,
        buffer: &mut Vec<u8>,
        key: &[u8; 32],
    ) -> Result<Option<Response>, Failure> {
        match poll_pipe(pipe.file()).map_err(|_| failure("Editor response transport failed"))? {
            PipePoll::Pending => return Ok(None),
            PipePoll::Closed => {
                return Err(failure(
                    "Editor disconnected; effect may require reconciliation",
                ))
            }
            PipePoll::Data(bytes) => buffer.extend_from_slice(&bytes),
        }
        if buffer.len() > LIMIT as usize + 40 {
            return Err(failure("Editor response limit"));
        }
        if buffer.len() < 8 {
            return Ok(None);
        }
        if buffer[..4] != custodian_ipc::MAGIC {
            return Err(failure("Editor response magic"));
        }
        let length = u32::from_le_bytes(buffer[4..8].try_into().map_err(|_| stale())?) as usize;
        if length > LIMIT as usize || buffer.len() > length + 40 {
            return Err(failure("Editor response limit"));
        }
        if buffer.len() < length + 40 {
            return Ok(None);
        }
        let reply = read_frame_limited(&mut std::io::Cursor::new(&buffer), key, LIMIT)
            .map_err(|_| failure("Editor response authentication/schema failed"))?;
        buffer.clear();
        Ok(Some(reply))
    }

    impl ApplicationExecutor for EditorSession<'_> {
        fn remeasure(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            control.check()?;
            if !self.window.current() {
                return Err(stale());
            }
            let mut measured = request.clone();
            measured.identity = self.identity.clone();
            measured.document_id = self.document_id.clone();
            if self.version.is_some() {
                let (version, _) = self.metadata(control)?;
                measured.document_version = format!("{version}:{}", self.digest);
            }
            Ok(measured)
        }
        fn execute(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            control.check()?;
            if request.document_id != self.document_id || request.identity != self.identity {
                return Err(stale());
            }
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
                ApplicationAction::EditBoundCells { .. } => {
                    return Err(failure(
                        "Spreadsheet edits are unavailable in the text editor",
                    ));
                }
                ApplicationAction::OpenBoundDocument => {
                    if self.version.is_some() {
                        return Err(stale());
                    }
                    let response = self.transact(
                        serde_json::json!({"kind":"bind","documentId":self.document_id}),
                        control,
                    )?;
                    let ResultData::Bound {
                        document_id,
                        version,
                        dirty,
                    } = response
                    else {
                        return Err(stale());
                    };
                    if document_id != self.document_id || dirty || version == 0 {
                        return Err(stale());
                    }
                    self.version = Some(version);
                }
                ApplicationAction::InspectBoundDocument => {
                    let response = self.transact(
                        serde_json::json!({"kind":"inspect","version":self.version}),
                        control,
                    )?;
                    let ResultData::Snapshot {
                        document_id,
                        version,
                        text,
                        digest,
                    } = response
                    else {
                        return Err(stale());
                    };
                    if document_id != self.document_id
                        || Some(version) != self.version
                        || text != self.text
                        || digest != self.digest
                        || sha(&text) != digest
                    {
                        return Err(stale());
                    }
                }
                ApplicationAction::EditBoundRange {
                    start,
                    end,
                    replacement,
                } => {
                    let start = usize::try_from(*start).map_err(|_| stale())?;
                    let end = usize::try_from(*end).map_err(|_| stale())?;
                    if start > end
                        || end > self.text.len()
                        || !self.text.is_char_boundary(start)
                        || !self.text.is_char_boundary(end)
                    {
                        return Err(stale());
                    }
                    let mut next = self.text.clone();
                    next.replace_range(start..end, replacement);
                    if next.len() > 65536 {
                        return Err(failure("Editor result limit"));
                    }
                    let digest = sha(&next);
                    let response=self.transact(serde_json::json!({"kind":"edit","version":self.version,"digest":self.digest,"start":start,"end":end,"replacement":replacement}),control)?;
                    let ResultData::Edited {
                        document_id,
                        version,
                        digest: received,
                    } = response
                    else {
                        return Err(stale());
                    };
                    let previous = self.version.ok_or_else(stale)?;
                    if document_id != self.document_id
                        || received != digest
                        || version < previous
                        || version > previous + 1
                    {
                        return Err(stale());
                    }
                    self.text = next;
                    self.digest = digest;
                    self.version = Some(version);
                }
                ApplicationAction::SaveBoundDocument => {
                    let response=self.transact(serde_json::json!({"kind":"save","version":self.version,"digest":self.digest}),control)?;
                    let ResultData::Saved {
                        document_id,
                        version,
                        digest,
                    } = response
                    else {
                        return Err(stale());
                    };
                    if document_id != self.document_id
                        || Some(version) != self.version
                        || digest != self.digest
                        || read_text(&self.target)? != self.text
                    {
                        return Err(stale());
                    }
                }
                ApplicationAction::VerifySavedOutput => {
                    let (version, dirty) = self.metadata(control)?;
                    if Some(version) != self.version
                        || dirty
                        || read_text(&self.target)? != self.text
                    {
                        return Err(stale());
                    }
                }
                ApplicationAction::CloseDocumentNormally => {
                    let response=self.transact(serde_json::json!({"kind":"close","version":self.version,"digest":self.digest}),control)?;
                    let ResultData::Closed { document_id } = response else {
                        return Err(stale());
                    };
                    if document_id != self.document_id {
                        return Err(stale());
                    }
                    self.closed = true;
                    self.close_window(control)?;
                }
            }
            Ok(ApplicationReceipt {
                identity: request.identity.clone(),
                document_id: request.document_id.clone(),
                document_name: request.document_name.clone(),
                action: request.action.clone(),
                verified: true,
                detail: "Bound editor operation verified; original files were not overwritten"
                    .into(),
            })
        }
    }
    impl Drop for EditorSession<'_> {
        fn drop(&mut self) {
            self.key.fill(0);
            // Dropping a stopped/revoked session cannot authorize WM_CLOSE on
            // a window where the user may have opened other work. The explicit
            // authenticated Close operation proves the bound tab and empty
            // window before normal close. Failed sessions remain for review.
        }
    }
    #[cfg(test)]
    mod close_tests {
        use super::*;
        struct Fixture(std::process::Child);
        impl Drop for Fixture {
            fn drop(&mut self) {
                if self.0.try_wait().ok().flatten().is_none() {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
        }
        fn delayed_process() -> (Fixture, (u32, u64)) {
            // Test-only child with no window, profile, document or user input.
            let executable = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let child = Command::new(executable)
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 17",
                ])
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let identity = custodian_winsec::apps::process_identity(child.id()).unwrap();
            (Fixture(child), (identity.pid, identity.creation_time))
        }
        #[test]
        fn normal_exit_can_use_the_existing_thirty_second_close_budget() {
            let (_child, identity) = delayed_process();
            let started = Instant::now();
            wait_for_process_exit(identity, identity, || {
                if started.elapsed() >= Duration::from_secs(30) {
                    Err(failure("Close deadline elapsed"))
                } else {
                    Ok(())
                }
            })
            .unwrap();
            assert!(started.elapsed() > Duration::from_secs(15));
            assert!(started.elapsed() < Duration::from_secs(30));
        }
        #[test]
        fn exit_wait_preserves_caller_deadline_and_authority_refusal() {
            let (mut child, identity) = delayed_process();
            let started = Instant::now();
            let error = wait_for_process_exit(identity, identity, || {
                if started.elapsed() >= Duration::from_millis(100) {
                    Err(failure("Caller deadline elapsed"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.detail, "Caller deadline elapsed");
            assert!(started.elapsed() < Duration::from_secs(2));
            let error = wait_for_process_exit(identity, identity, || {
                Err(Failure::new(
                    FailureClass::MissingAuthority,
                    "Revoked or stopped",
                ))
            })
            .unwrap_err();
            assert_eq!(error.class, FailureClass::MissingAuthority);
            assert!(child.0.try_wait().unwrap().is_none());
        }
    }
}
