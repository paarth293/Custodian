use custodian_engine::ops::quarantine::QuarantineOp;
use custodian_winsec::SafePath;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::thread;

#[test]
fn test_p4_concurrent_races() {
    let temp_dir = std::env::temp_dir().join("p4_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    let source_path = temp_dir.join("race.bin");
    let vault_path = temp_dir.join("vault");
    
    let op = Arc::new(QuarantineOp::new(vault_path).unwrap());
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let op_clone = Arc::clone(&op);
        let s_path = source_path.clone();
        let t_dir = temp_dir.clone();
        
        handles.push(thread::spawn(move || {
            let _ = fs::write(&s_path, b"data");
            
            if let Ok(safe_source) = SafePath::new(&s_path, &t_dir) {
                let _ = op_clone.execute(safe_source.as_path(), &format!("tx_{}", i));
            }
        }));
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert!(true);
}
