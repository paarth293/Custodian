//! Explicit real-editor fault fixtures. Test-only user interference is never a
//! production adapter operation or a selectable tool capability.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use custodian_core::{
        app_adapter::{
            execute_checked, ApplicationAction, ApplicationExecutor, ApplicationReceipt,
            ApplicationRequest,
        },
        app_authority::{
            AppPermission, ApplicationAuthority, ApplicationControl, ApplicationGrant,
        },
        app_journal::{ApplicationJournal, Event, ExpectedArtifact, RecoveryScope},
        app_privacy::{ApplicationIdentity, ApplicationPrivacyPolicy},
        editor_session::{EditorProgram, EditorSession},
        task_program::Failure,
    };
    use std::{
        path::PathBuf,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    fn permission(action: &ApplicationAction) -> AppPermission {
        match action {
            ApplicationAction::OpenBoundDocument => AppPermission::Open,
            ApplicationAction::InspectBoundDocument => AppPermission::Inspect,
            ApplicationAction::EditBoundRange { .. } | ApplicationAction::EditBoundCells { .. } => {
                AppPermission::Edit
            }
            ApplicationAction::SaveBoundDocument => AppPermission::Save,
            ApplicationAction::VerifySavedOutput => AppPermission::Verify,
            ApplicationAction::CloseDocumentNormally => AppPermission::Close,
        }
    }
    fn run(
        session: &mut EditorSession<'_>,
        authority: &ApplicationAuthority,
        cancel: &AtomicBool,
        journal: &ApplicationJournal,
        action: ApplicationAction,
    ) -> Result<(), String> {
        let effect = !matches!(
            action,
            ApplicationAction::InspectBoundDocument | ApplicationAction::VerifySavedOutput
        );
        let request = session.request(action);
        let grant = authority.issue_after_native_consent(
            &request,
            &[permission(&request.action)],
            Duration::from_secs(60),
        )?;
        let receipt =
            execute_checked(&request, authority, &grant, cancel, session).map_err(|e| e.detail)?;
        if effect {
            journal
                .record_receipt(&request, receipt)
                .map_err(|_| "Fixture receipt failed")?;
        }
        Ok(())
    }
    struct Interference<'a, 'b> {
        inner: &'a mut EditorSession<'b>,
        grant: &'a ApplicationGrant,
        after_save: bool,
        executed: bool,
    }
    impl ApplicationExecutor for Interference<'_, '_> {
        fn remeasure(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            let result = self.inner.remeasure(request, control)?;
            if !self.after_save {
                self.grant.revoke();
            }
            Ok(result)
        }
        fn execute(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            self.executed = true;
            let result = self.inner.execute(request, control)?;
            if self.after_save {
                self.grant.revoke();
            }
            Ok(result)
        }
    }
    fn fixture_normal_close(
        identity: &ApplicationIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let birth = identity
            .process_generation
            .split_once(':')
            .ok_or("Process generation required")?
            .1
            .parse()?;
        if !custodian_winsec::app_instance::WindowLease::recognizes(
            identity.pid,
            birth,
            identity.window,
            &identity.window_generation,
        ) {
            return Err("Fixture window changed; preserve it for manual review".into());
        }
        // Explicit test-user cleanup of this known synthetic window only.
        if !custodian_winsec::apps::request_close(identity.window as isize) {
            return Err("Normal fixture close failed".into());
        }
        let start = Instant::now();
        while custodian_winsec::apps::process_is_running(identity.pid, birth)? {
            if start.elapsed() > Duration::from_secs(20) {
                return Err("Fixture app remains for manual normal close".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
    let mode = std::env::args()
        .nth(1)
        .ok_or("Explicit fault scenario required")?;
    if ![
        "stale",
        "private-tab",
        "revoke-measure",
        "revoke-save",
        "stop",
        "forged-targets",
        "crash-save",
        "crash-recover",
        "duplicate-focus",
        "native-modal",
    ]
    .contains(&mode.as_str())
    {
        return Err("Unknown fixture scenario".into());
    }
    let root = PathBuf::from(
        std::env::var_os("CUSTODIAN_EDITOR_FAULT_ROOT").ok_or("Fresh fixture root required")?,
    );
    if !root.is_absolute() || root.try_exists()? {
        return Err("Fresh absolute fixture root required".into());
    }
    if mode == "crash-recover" {
        let status = std::process::Command::new(std::env::current_exe()?)
            .arg("crash-save")
            .status()?;
        if status.code() != Some(86) {
            return Err("Fixture did not crash at the intended receipt gap".into());
        }
        let identity: ApplicationIdentity =
            serde_json::from_slice(&std::fs::read(root.join("fixture-identity.json"))?)?;
        let source_id = custodian_winsec::fsops::file_identity(&root.join("original.txt"))?;
        let document_id = format!("file-document:{}:{}", source_id.volume, source_id.index);
        let files = custodian_core::file_tasks::FileTasks::new(root.join("file-receipts"));
        for _ in 0..2 {
            let result = custodian_core::app_recovery::inspect(&root, &document_id, &files)?;
            assert!(result.contains("matches the approved result"));
            assert!(result.contains("receipt is missing"));
            assert!(!root.join("unpublished.txt").exists());
        }
        std::fs::write(root.join("document.txt"), b"Later user edit after crash\n")?;
        let result = custodian_core::app_recovery::inspect(&root, &document_id, &files)?;
        assert!(result.contains("changed or unverified"));
        assert_eq!(
            std::fs::read(root.join("document.txt"))?,
            b"Later user edit after crash\n"
        );
        fixture_normal_close(&identity)?;
        assert_eq!(
            std::fs::read(root.join("original.txt"))?,
            b"Heading\nFirst line\nSecond line\n"
        );
        eprintln!("ACTUAL PROCESS CRASH VERIFIED: saved effect without receipt, restart inspection twice without replay, later edits preserved, normal fixture exit proved");
        return Ok(());
    }
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(".custodian-fixture-root"), b"")?;
    let original = "Heading\nFirst line\nSecond line\n";
    let revised = "Heading\nRevised line\nSecond line\n";
    let source = root.join("original.txt");
    let target = root.join("document.txt");
    let private = root.join("private.txt");
    let canary = "PRIVATE APP CANARY: never observe, log, capture or send to a model.";
    std::fs::write(&source, original)?;
    std::fs::write(&target, original)?;
    std::fs::write(&private, canary)?;
    let identity = custodian_winsec::fsops::file_identity(&source)?;
    let document_id = format!("file-document:{}:{}", identity.volume, identity.index);
    let program = EditorProgram::verify(
        &PathBuf::from(r"D:\paagu\Microsoft VS Code\Code.exe"),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../services/editor-adapter"),
    )
    .map_err(|e| e.detail)?;
    let authority = ApplicationAuthority::open(root.join("privacy.dpapi"))?;
    if mode == "private-tab" {
        let private_id = custodian_winsec::fsops::file_identity(&private)?;
        let mut policy = ApplicationPrivacyPolicy::default();
        policy.set_private_documents(&[format!(
            "file-document:{}:{}",
            private_id.volume, private_id.index
        )])?;
        authority.replace_policy(0, policy)?;
    }
    let journal = ApplicationJournal::create(&root.join("journal"), [5; 32])?;
    let expected = if mode == "revoke-save" || mode == "crash-save" || mode == "duplicate-focus" {
        revised
    } else {
        original
    };
    journal.append(Event::Scope {
        scope: RecoveryScope {
            source: source.clone(),
            destination: root.join("unpublished.txt"),
            source_identity: identity,
            source_digest: *blake3::hash(original.as_bytes()).as_bytes(),
            expected: ExpectedArtifact::Text {
                digest: *blake3::hash(expected.as_bytes()).as_bytes(),
                bytes: expected.len() as u64,
            },
        },
    })?;
    journal.append(Event::StagingPending {
        source_digest: *blake3::hash(original.as_bytes()).as_bytes(),
    })?;
    journal.append(Event::Staged {
        source_digest: *blake3::hash(original.as_bytes()).as_bytes(),
    })?;
    let launch = authority
        .issue_launch_after_native_consent(&program.executable(), Duration::from_secs(180))?;
    journal.append(Event::LaunchPending {
        executable_digest: *blake3::hash(program.executable().as_bytes()).as_bytes(),
    })?;
    let mut dispatch = journal.clone();
    let cancel = AtomicBool::new(false);
    let mut spawned = |pid, birth| {
        journal
            .append(Event::OwnedProcess { pid, birth })
            .map_err(|_| {
                Failure::new(
                    custodian_core::task_program::FailureClass::Unrecoverable,
                    "Fixture startup checkpoint",
                )
            })
    };
    let mut session = EditorSession::launch(
        &program,
        &root,
        &target,
        &authority,
        &launch,
        &cancel,
        &document_id,
        &mut dispatch,
        &mut spawned,
    )
    .map_err(|e| e.detail)?;
    run(
        &mut session,
        &authority,
        &cancel,
        &journal,
        ApplicationAction::OpenBoundDocument,
    )?;
    run(
        &mut session,
        &authority,
        &cancel,
        &journal,
        ApplicationAction::InspectBoundDocument,
    )?;
    let native_identity = session
        .request(ApplicationAction::InspectBoundDocument)
        .identity;
    std::fs::write(
        root.join("fixture-identity.json"),
        serde_json::to_vec(&native_identity)?,
    )?;
    let edit = ApplicationAction::EditBoundRange {
        start: 8,
        end: 18,
        replacement: "Revised line".into(),
    };
    if mode == "duplicate-focus" || mode == "native-modal" {
        let start = Instant::now();
        while !root.join("interference-ready").exists() {
            if start.elapsed() > Duration::from_secs(30) {
                fixture_normal_close(&native_identity)?;
                return Err("Explicit window/focus interference was not proved".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if mode == "native-modal" {
            let request = session.request(edit.clone());
            let grant = authority.issue_after_native_consent(
                &request,
                &[AppPermission::Edit],
                Duration::from_secs(60),
            )?;
            assert!(execute_checked(&request, &authority, &grant, &cancel, &mut session).is_err());
            assert_eq!(std::fs::read_to_string(&target)?, original);
            std::fs::write(root.join("modal-refused"), b"")?;
            let start = Instant::now();
            while !root.join("modal-dismissed").exists() {
                if start.elapsed() > Duration::from_secs(30) {
                    return Err("Synthetic modal cleanup unproved".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        for action in if mode == "native-modal" {
            vec![
                ApplicationAction::VerifySavedOutput,
                ApplicationAction::CloseDocumentNormally,
            ]
        } else {
            vec![
                edit.clone(),
                ApplicationAction::SaveBoundDocument,
                ApplicationAction::VerifySavedOutput,
                ApplicationAction::CloseDocumentNormally,
            ]
        } {
            run(&mut session, &authority, &cancel, &journal, action)?;
        }
        assert_eq!(
            std::fs::read_to_string(&target)?,
            if mode == "native-modal" {
                original
            } else {
                revised
            }
        );
    } else if mode == "revoke-save" || mode == "crash-save" {
        run(&mut session, &authority, &cancel, &journal, edit.clone())?;
    }
    if mode == "crash-save" {
        let request = session.request(ApplicationAction::SaveBoundDocument);
        let grant = authority.issue_after_native_consent(
            &request,
            &[AppPermission::Save],
            Duration::from_secs(60),
        )?;
        let _receipt = execute_checked(&request, &authority, &grant, &cancel, &mut session)
            .map_err(|e| e.detail)?;
        assert_eq!(std::fs::read_to_string(&target)?, revised);
        // Test-only hard exit skips Drop and intentionally omits the durable receipt.
        std::process::exit(86);
    }
    if mode == "private-tab" {
        use std::os::windows::process::CommandExt;
        let mut delegate = std::process::Command::new(program.executable())
            .arg("--user-data-dir")
            .arg(root.join("editor-profile"))
            .arg("--shared-data-dir")
            .arg(root.join("editor-shared"))
            .arg("--extensions-dir")
            .arg(root.join("editor-extensions"))
            .arg("--reuse-window")
            .arg(&private)
            .env_remove("CUSTODIAN_EDITOR_DESCRIPTOR")
            .creation_flags(0x08000000)
            .spawn()?;
        let start = Instant::now();
        while delegate.try_wait()?.is_none() {
            if start.elapsed() > Duration::from_secs(20) {
                return Err("Fixture tab switch did not finish".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    } else if mode == "stale" {
        std::fs::write(&target, b"Later user content\n")?;
    }
    if mode == "duplicate-focus" || mode == "native-modal" {
        // The complete scenario, including explicit normal cleanup, ran above.
    } else if mode == "stale" || mode == "private-tab" {
        let start = Instant::now();
        loop {
            let request = session.request(ApplicationAction::InspectBoundDocument);
            let grant = authority.issue_after_native_consent(
                &request,
                &[AppPermission::Inspect],
                Duration::from_secs(30),
            )?;
            match execute_checked(&request, &authority, &grant, &cancel, &mut session) {
                Err(error) => {
                    assert!(!error.detail.contains(canary));
                    break;
                }
                Ok(_) if start.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Ok(_) => return Err("Changed version/tab did not invalidate the binding".into()),
            }
        }
        let request = session.request(edit);
        let grant = authority.issue_after_native_consent(
            &request,
            &[AppPermission::Edit],
            Duration::from_secs(30),
        )?;
        assert!(execute_checked(&request, &authority, &grant, &cancel, &mut session).is_err());
        let expected = if mode == "stale" {
            "Later user content\n"
        } else {
            original
        };
        assert_eq!(std::fs::read_to_string(&target)?, expected);
        assert_eq!(std::fs::read_to_string(&private)?, canary);
        fixture_normal_close(&native_identity)?;
    } else if mode == "forged-targets" {
        let original_request = session.request(edit);
        for change in 0..5 {
            let mut request = original_request.clone();
            match change {
                0 => request.identity.process_generation.push_str("-reused"),
                1 => request.identity.window_generation.push_str("-reused"),
                2 => request.document_id.push_str("-substitute"),
                3 => request.document_version.push_str("-stale"),
                _ => request.identity.window += 1,
            }
            let grant = authority.issue_after_native_consent(
                &request,
                &[AppPermission::Edit],
                Duration::from_secs(30),
            )?;
            assert!(execute_checked(&request, &authority, &grant, &cancel, &mut session).is_err());
        }
        assert_eq!(std::fs::read_to_string(&target)?, original);
        run(
            &mut session,
            &authority,
            &cancel,
            &journal,
            ApplicationAction::VerifySavedOutput,
        )?;
        run(
            &mut session,
            &authority,
            &cancel,
            &journal,
            ApplicationAction::CloseDocumentNormally,
        )?;
    } else if mode == "stop" {
        cancel.store(true, std::sync::atomic::Ordering::Release);
        let request = session.request(edit);
        let grant = authority.issue_after_native_consent(
            &request,
            &[AppPermission::Edit],
            Duration::from_secs(30),
        )?;
        assert!(execute_checked(&request, &authority, &grant, &cancel, &mut session).is_err());
        assert_eq!(std::fs::read_to_string(&target)?, original);
        // Separate, explicit fixture-user consent for cleanup after the stopped request.
        let cleanup = AtomicBool::new(false);
        run(
            &mut session,
            &authority,
            &cleanup,
            &journal,
            ApplicationAction::VerifySavedOutput,
        )?;
        run(
            &mut session,
            &authority,
            &cleanup,
            &journal,
            ApplicationAction::CloseDocumentNormally,
        )?;
    } else {
        let request = session.request(if mode == "revoke-save" {
            ApplicationAction::SaveBoundDocument
        } else {
            edit
        });
        let grant = authority.issue_after_native_consent(
            &request,
            &[permission(&request.action)],
            Duration::from_secs(60),
        )?;
        let error = {
            let mut interfered = Interference {
                inner: &mut session,
                grant: &grant,
                after_save: mode == "revoke-save",
                executed: false,
            };
            let error = execute_checked(&request, &authority, &grant, &cancel, &mut interfered)
                .err()
                .ok_or("Revocation was ignored")?;
            assert_eq!(interfered.executed, mode == "revoke-save");
            error
        };
        if mode == "revoke-save" {
            let receipt = error
                .recovery_receipt
                .ok_or("Completed save evidence was lost")?;
            journal.record_receipt(&request, *receipt)?;
            assert_eq!(std::fs::read_to_string(&target)?, revised);
        } else {
            assert!(error.recovery_receipt.is_none());
            assert_eq!(std::fs::read_to_string(&target)?, original);
        }
        run(
            &mut session,
            &authority,
            &cancel,
            &journal,
            ApplicationAction::VerifySavedOutput,
        )?;
        run(
            &mut session,
            &authority,
            &cancel,
            &journal,
            ApplicationAction::CloseDocumentNormally,
        )?;
    }
    drop(session);
    drop(dispatch);
    drop(journal);
    let evidence = ApplicationJournal::recover(&root.join("journal"))?;
    assert!(!evidence.completed);
    assert!(!evidence.uncertain_effect);
    assert!(!root.join("unpublished.txt").exists());
    assert_eq!(std::fs::read_to_string(&source)?, original);
    eprintln!("REAL EDITOR ADVERSARIAL VERIFIED: {mode}; source preserved; no publication or automatic replay; normal fixture exit proved");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows required")
}
