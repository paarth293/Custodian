use custodian_engine::ops::quarantine::QuarantineOp;
use custodian_winsec::SafePath;
use std::fs;
use std::sync::Arc;
use std::thread;

#[test]
fn test_p4_concurrent_races() {
    let temp_dir = std::env::temp_dir().join("p4_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();
    
    let source_path = temp_dir.join("race.bin");
    let vault_path = temp_dir.join("vault");
    
    let op = Arc::new(QuarantineOp::new(vault_path.clone()).unwrap());
    
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
    
    // Whichever thread won each race, a quarantined payload must never be left
    // without its key sidecar.
    for entry in fs::read_dir(&vault_path).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "bin") {
            assert!(path.with_extension("key").exists(), "{path:?} has no key");
        }
    }
}
