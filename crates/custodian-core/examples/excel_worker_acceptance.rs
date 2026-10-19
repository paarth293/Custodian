//! Explicit fresh synthetic workbook fixture for the real supervised adapter.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use custodian_core::{
        app_adapter::{execute_checked, ApplicationAction, CellEdit, CellInput},
        app_authority::{AppPermission, ApplicationAuthority},
        app_worker::PendingDispatch,
        excel_session::{ExcelProgram, ExcelSession},
        task_program::{Failure, FailureClass},
        workbook,
    };
    use std::{io::Write, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
    struct Journal {
        root: PathBuf,
        index: usize,
    }
    impl PendingDispatch for Journal {
        fn record_pending(&mut self, nonce: &str, digest: [u8; 32]) -> Result<(), Failure> {
            let save = || -> std::io::Result<()> {
                let protected =
                    custodian_winsec::dpapi::protect(&serde_json::to_vec(&(nonce, digest))?)?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(self.root.join(format!("pending-{}.dpapi", self.index)))?;
                file.write_all(&protected)?;
                file.sync_all()
            };
            save()
                .map_err(|_| Failure::new(FailureClass::Unrecoverable, "Fixture journal failed"))?;
            self.index += 1;
            Ok(())
        }
    }
    let root = PathBuf::from(
        std::env::var_os("CUSTODIAN_EXCEL_WORKER_ACCEPTANCE_ROOT")
            .ok_or("Fresh explicit fixture path required")?,
    );
    if !root.is_absolute() || root.try_exists()? {
        return Err("Fresh absolute fixture root required".into());
    }
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(".custodian-fixture-root"), b"")?;
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/stage-4-completion/excel-native-01/output.xlsx");
    std::fs::copy(source, root.join("source.xlsx"))?;
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/custodian-appworker.exe")
        .canonicalize()?;
    let hash = blake3::hash(&std::fs::read(&worker)?).to_hex().to_string();
    let program = ExcelProgram::verify(
        &worker,
        &hash,
        &PathBuf::from(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"),
    )
    .map_err(|e| e.detail)?;
    let authority = ApplicationAuthority::open(root.join("app-privacy.dpapi"))?;
    let launch = authority
        .issue_launch_after_native_consent(program.executable(), Duration::from_secs(120))?;
    let cancel = AtomicBool::new(false);
    let mut journal = Journal {
        root: root.clone(),
        index: 0,
    };
    let mut spawned = |pid, birth| {
        std::fs::write(
            root.join("owned-process.json"),
            serde_json::to_vec(&serde_json::json!({"pid":pid,"birth":birth})).unwrap(),
        )
        .map_err(|_| Failure::new(FailureClass::Unrecoverable, "Fixture PID record"))
    };
    let mut session = ExcelSession::launch(
        &program,
        &root,
        &authority,
        &launch,
        &cancel,
        "synthetic-workbook-fixture",
        &mut journal,
        &mut spawned,
    )
    .map_err(|e| format!("launch: {}", e.detail))?;
    let sheet_id = session.sheet_id().to_string();
    for action in [
        ApplicationAction::OpenBoundDocument,
        ApplicationAction::InspectBoundDocument,
        ApplicationAction::EditBoundCells {
            sheet_id,
            cells: vec![
                CellEdit {
                    row: 1,
                    col: 1,
                    value: CellInput::Number {
                        decimal: "12".into(),
                    },
                },
                CellEdit {
                    row: 1,
                    col: 2,
                    value: CellInput::Text {
                        value: "=WEBSERVICE(\"https://invalid.example\")".into(),
                    },
                },
            ],
        },
        ApplicationAction::SaveBoundDocument,
        ApplicationAction::VerifySavedOutput,
        ApplicationAction::CloseDocumentNormally,
    ] {
        let permission = match &action {
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
            .map_err(|e| format!("{permission:?}: {}", e.detail))?;
        eprintln!("SUPERVISED EXCEL VERIFIED: {permission:?}");
    }
    drop(session);
    let snapshot = workbook::inspect(&std::fs::read(root.join("output.xlsx"))?)?;
    assert_eq!(
        snapshot
            .cells
            .iter()
            .find(|c| c.row == 3 && c.col == 1)
            .unwrap()
            .calculated,
        Some(20.0)
    );
    assert!(
        matches!(&snapshot.cells.iter().find(|c| c.row == 1 && c.col == 2).unwrap().value,
        custodian_winsec::excel::CellValue::Text { value } if value.starts_with("=WEBSERVICE"))
    );
    eprintln!("SUPERVISED EXCEL OUTPUT, SAFE TEXT, CALCULATION AND NORMAL EXIT VERIFIED");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows required")
}
