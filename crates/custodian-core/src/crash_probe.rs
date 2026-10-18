//! Fault barriers exist only in this crate's test executable. No release
//! binary reads these environment variables or offers a crash/debug command.
use crate::{
    file_tasks::{FileTasks, ReceiptState},
    instance::InstanceGuard,
    routine::RoutinePolicy,
    workflow::{self, Filter, Goal, Proposal, Selection, State, WorkflowStore},
};
use std::sync::atomic::AtomicBool;
use std::{
    fs, io,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
thread_local! { static DISK_FULL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
thread_local! { static FREE_BYTES: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) }; }

pub(crate) fn free_bytes(measured: u64) -> u64 {
    FREE_BYTES.with(|value| value.get().unwrap_or(measured))
}

#[test]
fn exact_approval_cannot_bypass_capacity_reserve_before_mutation() {
    use crate::file_tasks::FileAction;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("original.txt");
    let destination = root.path().join("copy.txt");
    fs::write(&source, b"preserve original").unwrap();
    let files = FileTasks::new(root.path().join("files"));
    let plan =
        crate::file_tasks::FileTaskPlan::prepare(FileAction::Copy, Some(&source), &destination)
            .unwrap();
    // Only the capacity measurement is substituted; approval snapshots,
    // policy, executor, file handles and receipt storage are the real path.
    FREE_BYTES.with(|value| value.set(Some(0)));
    let result = files.execute_plan(&plan);
    FREE_BYTES.with(|value| value.set(None));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("free-space reserve"));
    assert!(files.receipts().unwrap().is_empty());
    assert!(!destination.exists());
    assert_eq!(fs::read(&source).unwrap(), b"preserve original");
    files.execute_plan(&plan).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"preserve original");
}

pub(crate) fn checkpoint(point: &str) -> io::Result<()> {
    if point == "mid-copy" && DISK_FULL.with(|flag| flag.get()) {
        // ERROR_DISK_FULL is injected after real destination bytes were
        // written. Never exhaust the user's real volume to test this branch.
        return Err(io::Error::from_raw_os_error(112));
    }
    if std::env::var("CUSTODIAN_CRASH_POINT").as_deref() != Ok(point) {
        return Ok(());
    }
    let root = PathBuf::from(std::env::var_os("CUSTODIAN_CRASH_ROOT").expect("test root"));
    fs::write(root.join("barrier-ready"), point)?;
    loop {
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn injected_disk_full_after_real_write_preserves_partial_file_and_recovery_receipt() {
    let root = tempfile::tempdir().unwrap();
    for folder in ["source", "destination"] {
        fs::create_dir(root.path().join(folder)).unwrap();
    }
    let source = root.path().join("source/a.bin");
    let destination = root.path().join("destination/a.bin");
    fs::write(&source, vec![0x4b; 3 * 1024 * 1024]).unwrap();
    let files = FileTasks::new(root.path().join("files"));
    let plan = crate::file_tasks::FileTaskPlan::prepare(
        crate::file_tasks::FileAction::Copy,
        Some(&source),
        &destination,
    )
    .unwrap();
    DISK_FULL.with(|flag| flag.set(true));
    let result = files.execute_plan(&plan);
    DISK_FULL.with(|flag| flag.set(false));
    assert_eq!(result.unwrap_err().raw_os_error(), Some(112));
    assert_eq!(fs::metadata(&destination).unwrap().len(), 1024 * 1024);
    assert_eq!(fs::metadata(&source).unwrap().len(), 3 * 1024 * 1024);
    let reopened = FileTasks::new(root.path().join("files"));
    assert_eq!(
        reopened.receipts().unwrap()[0].state,
        ReceiptState::RecoveryRequired
    );
    assert!(reopened.undo_last().is_err());
    assert!(destination.exists());
}

#[test]
fn executor_child() {
    let Some(root) = std::env::var_os("CUSTODIAN_CRASH_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let _host = InstanceGuard::acquire(&root.join("state")).unwrap();
    let policy = RoutinePolicy::open(root.join("state/routine.dpapi")).unwrap();
    let cancel = AtomicBool::new(false);
    let goal = Goal {
        roots: vec![root.join("source")],
        destination: root.join("destination"),
        selection: Selection {
            filter: Filter::Any,
            largest: false,
        },
    };
    let prepared = workflow::prepare(
        &policy,
        policy.revision(),
        &goal,
        Proposal {
            steps: goal.selection.steps(),
            attempts: 1,
        },
        &cancel,
        &|_| true,
        &mut |_| {},
    )
    .unwrap();
    WorkflowStore::new(root.join("state/workflows"))
        .execute(
            &policy,
            prepared,
            &FileTasks::new(root.join("state/files")),
            &cancel,
            &mut |_| {},
        )
        .unwrap();
    panic!("child should be killed at the requested executor barrier");
}

#[test]
fn actual_process_death_at_write_boundaries_is_reconciled_without_replay() {
    for point in [
        "before-mutation",
        "mid-copy",
        "child-persisted",
        "before-parent-completion",
    ] {
        let root = tempfile::tempdir().unwrap();
        for folder in ["state", "source", "destination"] {
            fs::create_dir(root.path().join(folder)).unwrap();
        }
        // Two files prove restart never starts a subsequent step either.
        fs::write(
            root.path().join("source/a.bin"),
            vec![0x5a; 3 * 1024 * 1024],
        )
        .unwrap();
        fs::write(
            root.path().join("source/b.bin"),
            b"never copied after death",
        )
        .unwrap();
        RoutinePolicy::open(root.path().join("state/routine.dpapi"))
            .unwrap()
            .update_permissions(0, false, Some(true))
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_probe::executor_child", "--nocapture"])
            .env("CUSTODIAN_CRASH_ROOT", root.path())
            .env("CUSTODIAN_CRASH_POINT", point)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let ready = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !root.path().join("barrier-ready").exists() {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "executor exited before {point}"
                );
                assert!(Instant::now() < deadline, "executor timeout at {point}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }));
        child.kill().unwrap();
        child.wait().unwrap();
        ready.unwrap();
        // Restart admission and reload the real sealed policy, parent journal
        // and child receipts. Inspection itself has no executor/replay branch.
        let _host = InstanceGuard::acquire(&root.path().join("state")).unwrap();
        let files = FileTasks::new(root.path().join("state/files"));
        let store = WorkflowStore::new(root.path().join("state/workflows"));
        let receipts = files.receipts().unwrap();
        let all_children = point == "before-parent-completion";
        assert_eq!(receipts.len(), if all_children { 2 } else { 1 }, "{point}");
        assert_eq!(
            receipts[0].state,
            if point == "child-persisted" || all_children {
                ReceiptState::Completed
            } else {
                ReceiptState::Prepared
            }
        );
        let before: Vec<_> = fs::read_dir(root.path().join("destination"))
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                (p.clone(), fs::read(p).unwrap())
            })
            .collect();
        if point == "before-mutation" {
            assert!(before.is_empty());
        }
        if point == "mid-copy" {
            assert_eq!(before[0].1.len(), 1024 * 1024);
        }
        let record = store.inspect(&files).unwrap().unwrap();
        assert_eq!(record.state, State::RecoveryRequired);
        assert_eq!(
            record.steps.iter().filter(|s| s.verified).count(),
            if all_children {
                2
            } else {
                usize::from(point == "child-persisted")
            }
        );
        assert_eq!(record.steps[1].destination.exists(), all_children);
        for (path, bytes) in &before {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
        assert_eq!(
            fs::read_dir(root.path().join("destination"))
                .unwrap()
                .count(),
            before.len()
        );
        assert_eq!(
            fs::metadata(root.path().join("source/a.bin"))
                .unwrap()
                .len(),
            3 * 1024 * 1024
        );
        let undo = store.undo_latest(&files, &AtomicBool::new(false));
        if point == "child-persisted" || all_children {
            assert_eq!(undo.unwrap().state, State::Undone);
        } else {
            assert!(
                undo.is_err(),
                "Unverified receipts must not authorize deletion"
            );
        }
        if point == "mid-copy" {
            assert_eq!(fs::read(&before[0].0).unwrap(), before[0].1);
        }
    }
}
