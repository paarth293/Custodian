use proptest::prelude::*;
use custodian_engine::ops::quarantine::QuarantineOp;
use custodian_winsec::SafePath;
use std::fs::{self, File};
use std::io::Write;

proptest! {
    #[test]
    fn test_quarantine_roundtrip_property(data in any::<Vec<u8>>()) {
        let temp_dir = std::env::temp_dir().join("p1_test");
        fs::create_dir_all(&temp_dir).unwrap();
        
        let source_path = temp_dir.join("source.bin");
        let vault_path = temp_dir.join("vault");
        
        {
            let mut file = File::create(&source_path).unwrap();
            file.write_all(&data).unwrap();
        }
        
        let op = QuarantineOp::new(vault_path.clone()).unwrap();
        
        let safe_source = SafePath::new(&source_path, &temp_dir).unwrap();
        
        op.execute(safe_source.as_path(), "tx_prop").unwrap();
        assert!(!source_path.exists());
        
        op.inverse(safe_source.as_path(), "tx_prop").unwrap();
        
        let restored = fs::read(&source_path).unwrap();
        assert_eq!(restored, data);
    }
}
