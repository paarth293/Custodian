//! Explicit disposable native Excel fixture. No production/test-mode switch.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use custodian_winsec::excel::{CellValue, ExcelSession};
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    let root = PathBuf::from(
        std::env::var_os("CUSTODIAN_EXCEL_ACCEPTANCE_ROOT").ok_or("Fresh fixture root required")?,
    );
    if !root.is_absolute() || root.try_exists()? {
        return Err("Fresh absolute fixture required".into());
    }
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(".custodian-fixture-root"), b"")?;
    let mut excel = ExcelSession::start(&[])?;
    let identity = excel.identity().clone();
    std::fs::write(
        root.join("owned-process.json"),
        serde_json::to_vec(
            &serde_json::json!({"pid":identity.pid,"birth":identity.creation_time}),
        )?,
    )?;
    excel.new_task_workbook()?;
    let mut expected = excel.snapshot()?;
    for (row, value) in [
        (1, CellValue::Number { value: 7.0 }),
        (2, CellValue::Number { value: 8.0 }),
        (
            3,
            CellValue::Sum {
                first_row: 1,
                first_col: 1,
                last_row: 2,
                last_col: 1,
            },
        ),
    ] {
        expected = excel.edit_cell(&expected, row, 1, &value)?;
    }
    assert_eq!(
        expected
            .cells
            .iter()
            .find(|c| c.row == 3)
            .unwrap()
            .calculated,
        Some(15.0)
    );
    let output = root.join("output.xlsx");
    excel.save_new(&output, &expected)?;
    std::fs::write(
        root.join("snapshot.json"),
        serde_json::to_vec_pretty(&expected)?,
    )?;
    excel.close(&expected)?;
    drop(excel);
    let wait = |pid, birth| -> Result<(), Box<dyn std::error::Error>> {
        let start = Instant::now();
        while custodian_winsec::apps::process_is_running(pid, birth)? {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Owned Excel did not exit".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    };
    wait(identity.pid, identity.creation_time)?;
    eprintln!("NATIVE EXCEL CREATED, CALCULATED, SAVED AND CLOSED");
    let mut reopened = ExcelSession::start(&[])?;
    let second = reopened.identity().clone();
    reopened.open_task_copy(&output)?;
    assert_eq!(reopened.snapshot()?, expected);
    reopened.close(&expected)?;
    drop(reopened);
    wait(second.pid, second.creation_time)?;
    eprintln!("NATIVE EXCEL REOPENED VALUES/FORMULA/CALCULATION AND NORMAL EXIT VERIFIED");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows native fixture required")
}
