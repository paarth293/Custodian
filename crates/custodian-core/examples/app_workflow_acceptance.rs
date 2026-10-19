//! Real coordinated app task fixture, including publication, restart and undo.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use custodian_core::{
        app_adapter::{CellEdit, CellInput},
        app_authority::ApplicationAuthority,
        app_journal::ApplicationJournal,
        app_workflow::{execute_after_native_consent, ApplicationGoal, ApplicationPlan, Programs},
        coordinator::{TaskCoordinator, TaskState},
        editor_session::EditorProgram,
        excel_session::ExcelProgram,
        file_tasks::FileTasks,
    };
    use custodian_winsec::excel::{CellValue, ExcelSession};
    use std::path::PathBuf;
    fn wait_exit(
        identity: &custodian_winsec::apps::ProcessIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        while custodian_winsec::apps::process_is_running(identity.pid, identity.creation_time)? {
            if start.elapsed() > std::time::Duration::from_secs(15) {
                return Err("Synthetic Excel remains open for manual review".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Ok(())
    }
    struct Canary {
        child: std::process::Child,
        root: PathBuf,
    }
    impl Canary {
        fn finish(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            std::fs::write(self.root.join("canary-finish"), b"")?;
            let start = std::time::Instant::now();
            loop {
                if let Some(status) = self.child.try_wait()? {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err("Synthetic canary preservation/cleanup failed".into())
                    };
                }
                if start.elapsed() > std::time::Duration::from_secs(30) {
                    return Err("Canary cleanup remains unproved; preserve for review".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
    impl Drop for Canary {
        fn drop(&mut self) {
            if let Err(error) = self.finish() {
                eprintln!("{error}");
            }
        }
    }
    let root = PathBuf::from(
        std::env::var_os("CUSTODIAN_APP_WORKFLOW_ROOT")
            .ok_or("Fresh explicit fixture root required")?,
    );
    let mode = std::env::args().nth(1).ok_or("editor or excel required")?;
    let control_case = std::env::args().nth(2);
    if control_case
        .as_deref()
        .is_some_and(|case| !["pause-resume", "pause-stop"].contains(&case))
        || (control_case.is_some() && mode != "editor")
    {
        return Err("Unknown native control fixture".into());
    }
    if mode == "excel-canary" {
        if !root.is_absolute() || !root.join(".custodian-fixture-root").is_file() {
            return Err("Marked fixture root required".into());
        }
        let mut canary = ExcelSession::start(&custodian_winsec::app_instance::process_ids()?)?;
        std::fs::write(
            root.join("canary-owned-process.json"),
            serde_json::to_vec(
                &serde_json::json!({"pid":canary.identity().pid,"birth":canary.identity().creation_time}),
            )?,
        )?;
        canary.new_task_workbook()?;
        let expected = canary.snapshot()?;
        let expected = canary.edit_cell(
            &expected,
            1,
            1,
            &CellValue::Text {
                value: "Preserve unrelated workbook".into(),
            },
        )?;
        assert!(!canary.metadata()?.1);
        std::fs::write(root.join("canary-ready"), b"")?;
        let start = std::time::Instant::now();
        while !root.join("canary-finish").exists()
            && start.elapsed() < std::time::Duration::from_secs(180)
        {
            // Keep the independent fixture STA servicing COM while the product runs.
            assert!(canary.snapshot()? == expected, "Unrelated workbook changed");
            assert!(
                !canary.metadata()?.1,
                "Unrelated unsaved workbook was saved"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        canary.save_new(&root.join("unrelated-canary.xlsx"), &expected)?;
        let identity = canary.identity().clone();
        canary.close(&expected)?;
        drop(canary);
        wait_exit(&identity)?;
        eprintln!("UNRELATED UNSAVED WORKBOOK PRESERVED THROUGH TASK AND CLOSED NORMALLY");
        return Ok(());
    }
    if !root.is_absolute() || root.try_exists()? {
        return Err("Fresh absolute root required".into());
    }
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(".custodian-fixture-root"), b"")?;
    let authority = ApplicationAuthority::open(root.join("privacy.dpapi"))?;
    let mut unrelated = None;
    let (editor, excel, source, destination, goal) = if mode == "editor" {
        let editor = EditorProgram::verify(
            &PathBuf::from(r"D:\paagu\Microsoft VS Code\Code.exe"),
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../services/editor-adapter"),
        )
        .map_err(|e| e.detail)?;
        let source = root.join("original.txt");
        std::fs::write(&source, "Heading\n😀First line\nSecond line\n")?;
        (
            Some(editor),
            None,
            source,
            root.join("published.txt"),
            ApplicationGoal::Text {
                find: "First line".into(),
                replacement: "Revised line".into(),
            },
        )
    } else if mode == "excel" {
        let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/custodian-appworker.exe")
            .canonicalize()?;
        let hash = blake3::hash(&std::fs::read(&worker)?).to_hex().to_string();
        let excel = ExcelProgram::verify(
            &worker,
            &hash,
            &PathBuf::from(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"),
        )
        .map_err(|e| e.detail)?;
        let source = root.join("original.xlsx");
        // Explicit fixture-user setup; these are not product capabilities.
        let child = std::process::Command::new(std::env::current_exe()?)
            .arg("excel-canary")
            .spawn()?;
        let canary = Canary {
            child,
            root: root.clone(),
        };
        let start = std::time::Instant::now();
        while !root.join("canary-ready").exists() {
            if start.elapsed() > std::time::Duration::from_secs(30) {
                return Err("Independent canary startup failed".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        unrelated = Some(canary);
        let mut fixture = ExcelSession::start(&custodian_winsec::app_instance::process_ids()?)?;
        fixture.new_task_workbook()?;
        let mut expected = fixture.snapshot()?;
        for (row, col, value) in [
            (1, 1, CellValue::Number { value: 7.0 }),
            (2, 1, CellValue::Number { value: 8.0 }),
            (
                3,
                1,
                CellValue::Sum {
                    first_row: 1,
                    first_col: 1,
                    last_row: 2,
                    last_col: 1,
                },
            ),
            (
                16,
                16,
                CellValue::Text {
                    value: "Out-of-scope sentinel".into(),
                },
            ),
        ] {
            expected = fixture.edit_cell(&expected, row, col, &value)?;
        }
        fixture.save_new(&source, &expected)?;
        let identity = fixture.identity().clone();
        fixture.close(&expected)?;
        drop(fixture);
        wait_exit(&identity)?;
        (
            None,
            Some(excel),
            source,
            root.join("published.xlsx"),
            ApplicationGoal::Cells {
                edits: vec![CellEdit {
                    row: 1,
                    col: 1,
                    value: CellInput::Number {
                        decimal: "12".into(),
                    },
                }],
            },
        )
    } else {
        return Err("Unknown fixture mode".into());
    };
    let executable = editor
        .as_ref()
        .map(EditorProgram::executable)
        .unwrap_or_else(|| excel.as_ref().unwrap().executable().into());
    let plan = ApplicationPlan::prepare(&source, &destination, goal, &executable, &authority)
        .map_err(|e| e.detail)?;
    let original = std::fs::read(&source)?;
    let tasks_path = root.join("tasks.dpapi");
    let coordinator = std::sync::Arc::new(TaskCoordinator::open(tasks_path.clone())?);
    let (id, cancel) = coordinator.create_task(
        "Synthetic real app workflow".into(),
        TaskState::Preparing,
        &[source.clone(), destination.clone()],
    )?;
    let files_root = root.join("file-receipts");
    let files = FileTasks::new(files_root.clone());
    let programs = match (&editor, &excel) {
        (Some(p), _) => Programs::Editor(p),
        (_, Some(p)) => Programs::Excel(p),
        _ => unreachable!(),
    };
    let observer_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observer = control_case.as_ref().map(|case| {
        let coordinator = coordinator.clone();
        let id = id.clone();
        let done = observer_done.clone();
        let stop = case == "pause-stop";
        std::thread::spawn(move || -> Result<(), String> {
            let start = std::time::Instant::now();
            let mut requested = false;
            while !done.load(std::sync::atomic::Ordering::Acquire) {
                if start.elapsed() > std::time::Duration::from_secs(90) {
                    coordinator.cancel_task(&id);
                    return Err("Native control fixture deadline exceeded".into());
                }
                let checkpoint = coordinator.get_task(&id).and_then(|t| t.execution);
                if !requested && checkpoint.as_ref().is_some_and(|c| c.in_flight == Some(5)) {
                    coordinator
                        .pause_program(&id, true)
                        .map_err(|e| e.to_string())?;
                    requested = true;
                }
                if let Some(checkpoint) = checkpoint.filter(|c| c.paused) {
                    if checkpoint.outputs.len() != 6 {
                        coordinator.cancel_task(&id);
                        return Err("Pause did not land after normal app close".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let current = coordinator
                        .get_task(&id)
                        .and_then(|t| t.execution)
                        .ok_or("Paused evidence lost")?;
                    if !current.paused || current.outputs.len() != 6 {
                        coordinator.cancel_task(&id);
                        return Err("Effects advanced during pause".into());
                    }
                    if stop {
                        if !coordinator.cancel_task(&id) {
                            return Err("Paused task could not stop".into());
                        }
                    } else {
                        coordinator
                            .pause_program(&id, false)
                            .map_err(|e| e.to_string())?;
                    }
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err("Workflow ended before native pause was observed".into())
        })
    });
    let result = execute_after_native_consent(
        &plan,
        programs,
        &authority,
        &coordinator,
        &id,
        &files,
        &cancel,
    );
    observer_done.store(true, std::sync::atomic::Ordering::Release);
    if let Some(observer) = observer {
        observer
            .join()
            .map_err(|_| "Native control observer panicked")??;
    }
    if control_case.as_deref() == Some("pause-stop") {
        assert!(result.is_err());
        assert!(!destination.exists());
        assert!(files.receipts()?.is_empty());
        assert_eq!(std::fs::read(&source)?, original);
        let evidence = ApplicationJournal::recover(&plan.workspace.join("journal"))?;
        assert!(!evidence.completed);
        assert!(!evidence.uncertain_effect);
        let inspection =
            custodian_core::app_recovery::inspect(&plan.workspace, &plan.document_id, &files)?;
        assert!(inspection.contains("matches the approved result"));
        drop(coordinator);
        let restarted = TaskCoordinator::open(tasks_path)?;
        assert!(!restarted
            .get_task(&id)
            .ok_or("Stopped task missing after restart")?
            .state
            .is_active());
        assert!(!destination.exists());
        eprintln!("REAL APP PAUSE/STOP VERIFIED: normal close before pause, no queued publication, approved stage retained, restart without replay");
        return Ok(());
    }
    let checkpoint = result.map_err(|e| {
        if let Some(task) = coordinator.get_task(&id) {
            if let Some(execution) = task.execution {
                eprintln!("Failed at native step {:?}", execution.in_flight);
            }
        }
        e.detail
    })?;
    assert!(checkpoint.verified);
    assert_eq!(checkpoint.outputs.len(), 8);
    assert_eq!(std::fs::read(&source)?, original);
    let evidence = ApplicationJournal::recover(&plan.workspace.join("journal"))?;
    assert!(evidence.completed);
    assert!(!evidence.uncertain_effect);
    let receipt = files
        .receipts()?
        .into_iter()
        .next()
        .ok_or("Publication receipt missing")?;
    files.verify_copy(&receipt)?;
    if let Some(mut canary) = unrelated {
        // Independent reopen of the published artifact while unrelated work stays open.
        let mut reopened = ExcelSession::start(&custodian_winsec::app_instance::process_ids()?)?;
        reopened.open_task_copy(&destination)?;
        let saved = reopened.snapshot()?;
        assert!(saved.cells.iter().any(|c| c.row == 16
            && c.col == 16
            && c.value
                == CellValue::Text {
                    value: "Out-of-scope sentinel".into()
                }));
        assert!(saved
            .cells
            .iter()
            .any(|c| c.row == 3 && c.col == 1 && c.calculated == Some(20.0)));
        let identity = reopened.identity().clone();
        reopened.close(&saved)?;
        drop(reopened);
        wait_exit(&identity)?;
        canary.finish()?;
        eprintln!(
            "UNRELATED UNSAVED WORKBOOK, OUT-OF-SCOPE CELL, RECALCULATION AND REAL REOPEN VERIFIED"
        );
    }
    drop(coordinator);
    drop(files);
    let _restarted_tasks = TaskCoordinator::open(tasks_path)?;
    let files = FileTasks::new(files_root);
    files.verify_copy(&receipt)?;
    files.undo(&receipt.id)?;
    assert!(!destination.exists());
    assert_eq!(std::fs::read(&source)?, original);
    eprintln!("COORDINATED {mode} VERIFIED: 8 steps, original preserved, publication, sealed restart evidence and safe undo");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows required")
}
