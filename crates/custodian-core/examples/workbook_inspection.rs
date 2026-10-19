//! Read-only qualification of an explicitly provided fixture workbook.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("explicit fixture required")?;
    let bytes = std::fs::read(path)?;
    let snapshot = custodian_core::workbook::inspect(&bytes)?;
    let recalculated = custodian_core::workbook::recalculate(&snapshot)?;
    assert_eq!(snapshot, recalculated);
    println!("INDEPENDENT XLSX VERIFIED: {} cells", snapshot.cells.len());
    Ok(())
}
