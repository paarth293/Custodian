//! Property P4: concurrent quarantines never lose or corrupt data.

use custodian_engine::ops::quarantine::QuarantineOp;
use std::fs;
use std::sync::Arc;
use std::thread;

#[test]
fn concurrent_quarantines_are_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let op = Arc::new(QuarantineOp::new(dir.path().join("vault")).unwrap());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let op = Arc::clone(&op);
            let source = dir.path().join(format!("race_{i}.bin"));
            thread::spawn(move || {
                let content = format!("payload {i}");
                fs::write(&source, &content).unwrap();
                let id = format!("tx_{i}");
                op.execute(&source, &id).unwrap();
                op.inverse(&source, &id).unwrap();
                assert_eq!(fs::read_to_string(&source).unwrap(), content);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
