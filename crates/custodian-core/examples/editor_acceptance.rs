//! Explicit native application fixture, never called by a production request.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use custodian_core::{
        app_adapter::{execute_checked, ApplicationAction},
        app_authority::{AppPermission, ApplicationAuthority},
        app_worker::PendingDispatch,
        editor_session::{EditorProgram, EditorSession},
        task_program::{Failure, FailureClass},
    };
    use std::{io::Write, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    struct Journal {
        root: PathBuf,
        count: usize,
    }
    impl PendingDispatch for Journal {
        fn record_pending(&mut self, nonce: &str, digest: [u8; 32]) -> Result<(), Failure> {
            self.count += 1;
            let bytes =
                custodian_winsec::dpapi::protect(&serde_json::to_vec(&(nonce, digest)).unwrap())
                    .map_err(|_| Failure::new(FailureClass::Unrecoverable, "fixture checkpoint"))?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.root.join(format!("pending-{}.dpapi", self.count)))
                .unwrap();
            file.write_all(&bytes).unwrap();
            file.sync_all().unwrap();
            Ok(())
        }
    }
    let root = PathBuf::from(
        std::env::var_os("CUSTODIAN_EDITOR_ACCEPTANCE_ROOT")
            .ok_or("Explicit fresh fixture path required")?,
    );
    if !root.is_absolute() || root.try_exists()? {
        return Err("Fresh absolute fixture path required".into());
    }
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(".custodian-fixture-root"), b"")?;
    let source = "Heading\n😀First line\nSecond line\n";
    let target = root.join("document.txt");
    std::fs::write(&target, source)?;
    let program = EditorProgram::verify(
        &PathBuf::from(r"D:\paagu\Microsoft VS Code\Code.exe"),
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../services/editor-adapter"),
    )
    .map_err(|e| format!("program: {}", e.detail))?;
    let authority = ApplicationAuthority::open(root.join("app-privacy.dpapi"))?;
    let launch = authority
        .issue_launch_after_native_consent(&program.executable(), Duration::from_secs(120))?;
    let cancel = AtomicBool::new(false);
    let mut journal = Journal {
        root: root.clone(),
        count: 0,
    };
    let mut spawned = |pid, birth| {
        std::fs::write(
            root.join("owned-process.json"),
            serde_json::to_vec(&serde_json::json!({"pid":pid,"birth":birth})).unwrap(),
        )
        .unwrap();
        Ok(())
    };
    let mut session = EditorSession::launch(
        &program,
        &root,
        &target,
        &authority,
        &launch,
        &cancel,
        "synthetic-editor-fixture",
        &mut journal,
        &mut spawned,
    )
    .map_err(|e| format!("launch: {}", e.detail))?;
    let start = "Heading\n😀".len() as u64;
    for action in [
        ApplicationAction::OpenBoundDocument,
        ApplicationAction::InspectBoundDocument,
        ApplicationAction::EditBoundRange {
            start,
            end: start + 10,
            replacement: "Revised line".into(),
        },
        ApplicationAction::SaveBoundDocument,
        ApplicationAction::VerifySavedOutput,
        ApplicationAction::CloseDocumentNormally,
    ] {
        let permission = match action {
            ApplicationAction::OpenBoundDocument => AppPermission::Open,
            ApplicationAction::InspectBoundDocument => AppPermission::Inspect,
            ApplicationAction::EditBoundRange { .. } | ApplicationAction::EditBoundCells { .. } => {
                AppPermission::Edit
            }
            ApplicationAction::SaveBoundDocument => AppPermission::Save,
            ApplicationAction::VerifySavedOutput => AppPermission::Verify,
            ApplicationAction::CloseDocumentNormally => AppPermission::Close,
        };
        let request = session.request(action);
        let grant = authority.issue_after_native_consent(
            &request,
            &[permission],
            Duration::from_secs(60),
        )?;
        execute_checked(&request, &authority, &grant, &cancel, &mut session)
            .map_err(|e| format!("action {:?}: {}", request.action, e.detail))?;
        eprintln!("REAL EDITOR VERIFIED: {:?}", permission);
    }
    drop(session);
    assert_eq!(
        std::fs::read_to_string(&target)?,
        "Heading\n😀Revised line\nSecond line\n"
    );
    eprintln!("REAL EDITOR SAVED BYTES AND NORMAL CLOSE VERIFIED");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows native fixture required")
}
