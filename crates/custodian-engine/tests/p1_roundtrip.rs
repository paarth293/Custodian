//! Property P1: quarantine followed by undo restores the exact bytes.

use custodian_engine::ops::quarantine::QuarantineOp;
use custodian_winsec::SafePath;
use proptest::prelude::*;
use std::fs;

proptest! {
    #[test]
    fn quarantine_roundtrip_preserves_content(data in any::<Vec<u8>>()) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        fs::write(&source, &data).unwrap();

        let op = QuarantineOp::new(dir.path().join("vault")).unwrap();
        let safe_source = SafePath::new(&source, dir.path()).unwrap();

        op.execute(safe_source.as_path(), "tx_prop").unwrap();
        prop_assert!(!source.exists());

        op.inverse(safe_source.as_path(), "tx_prop").unwrap();
        prop_assert_eq!(fs::read(&source).unwrap(), data);
    }
}
