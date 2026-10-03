use custodian_winsec::safe_place::SafePlace;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Read, Write};

/// Transaction ids become file names inside the vault, so they must not be able
/// to carry path separators or `..` out of it.
fn validate_id(id: &str) -> io::Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid transaction id"))
    }
}

pub struct QuarantineOp {
    vault: SafePlace,
}

impl QuarantineOp {
    pub fn new(vault_path: PathBuf) -> io::Result<Self> {
        let vault = SafePlace::new(vault_path)?;
        Ok(Self { vault })
    }

    pub fn execute(&self, source: &Path, id: &str) -> io::Result<()> {
        validate_id(id)?;
        let mut file = fs::File::open(source)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let mut encrypted = data.clone();
        encrypted.reverse();

        let vault_file = id.to_string() + ".bin";
        let vault_path = self.vault.root().join(&vault_file);
        fs::write(&vault_path, &encrypted)?;

        let sidecar_file = id.to_string() + ".key";
        let key_material = b"mock_key_123";
        self.vault.write_encrypted(&sidecar_file, key_material)?;

        fs::remove_file(source)?;
        
        Ok(())
    }

    pub fn inverse(&self, target: &Path, id: &str) -> io::Result<()> {
        validate_id(id)?;
        let sidecar_file = id.to_string() + ".key";
        let _key = self.vault.read_decrypted(&sidecar_file)?;

        let vault_file = id.to_string() + ".bin";
        let vault_path = self.vault.root().join(&vault_file);
        let mut data = fs::read(&vault_path)?;

        data.reverse();

        // Never clobber whatever has appeared at the original location since.
        let mut restored = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        restored.write_all(&data)?;
        restored.sync_all()?;

        fs::remove_file(&vault_path)?;
        fs::remove_file(self.vault.root().join(&sidecar_file))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_traversal_ids_and_never_overwrites() {
        let temp_dir = std::env::temp_dir().join("quarantine_test_ids");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let source = temp_dir.join("a.txt");
        fs::write(&source, b"payload").unwrap();
        let op = QuarantineOp::new(temp_dir.join("vault")).unwrap();

        for bad in ["", "../escape", "a/b", "a\\b", "..", "tx.bin"] {
            let err = op.execute(&source, bad).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        assert!(source.exists());

        op.execute(&source, "tx1").unwrap();
        fs::write(&source, b"newer file").unwrap();
        let err = op.inverse(&source, "tx1").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&source).unwrap(), b"newer file");
    }

    #[test]
    fn test_quarantine_roundtrip() {
        let temp_dir = std::env::temp_dir().join("quarantine_test");
        fs::create_dir_all(&temp_dir).unwrap();
        
        let source = temp_dir.join("secret.txt");
        fs::write(&source, b"confidential data").unwrap();
        
        let vault_path = temp_dir.join("vault");
        let op = QuarantineOp::new(vault_path).unwrap();
        
        op.execute(&source, "tx123").unwrap();
        assert!(!source.exists());
        
        op.inverse(&source, "tx123").unwrap();
        assert!(source.exists());
        let restored = fs::read(&source).unwrap();
        assert_eq!(restored, b"confidential data");
    }
}
