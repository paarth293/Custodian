use custodian_winsec::safe_place::SafePlace;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Read};

pub struct QuarantineOp {
    vault: SafePlace,
}

impl QuarantineOp {
    pub fn new(vault_path: PathBuf) -> io::Result<Self> {
        let vault = SafePlace::new(vault_path)?;
        Ok(Self { vault })
    }

    pub fn execute(&self, source: &Path, id: &str) -> io::Result<()> {
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
        let sidecar_file = id.to_string() + ".key";
        let _key = self.vault.read_decrypted(&sidecar_file)?;

        let vault_file = id.to_string() + ".bin";
        let vault_path = self.vault.root().join(&vault_file);
        let mut data = fs::read(&vault_path)?;

        data.reverse();

        fs::write(target, &data)?;
        
        fs::remove_file(&vault_path)?;
        fs::remove_file(self.vault.root().join(&sidecar_file))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
