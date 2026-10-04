use custodian_winsec::safe_place::SafePlace;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Moves files into an encrypted vault and restores them on undo.
///
/// Placeholder crypto: the payload transform and key material are stand-ins
/// until the DPAPI-backed implementation lands (see `HONESTY.md`).
pub struct QuarantineOp {
    vault: SafePlace,
}

impl QuarantineOp {
    pub fn new(vault_path: PathBuf) -> io::Result<Self> {
        Ok(Self {
            vault: SafePlace::new(vault_path)?,
        })
    }

    pub fn execute(&self, source: &Path, id: &str) -> io::Result<()> {
        let mut data = fs::read(source)?;
        data.reverse();

        fs::write(self.payload_path(id), &data)?;
        self.vault
            .write_encrypted(&Self::key_name(id), b"placeholder-key")?;
        fs::remove_file(source)
    }

    pub fn inverse(&self, target: &Path, id: &str) -> io::Result<()> {
        let key_name = Self::key_name(id);
        let _key = self.vault.read_decrypted(&key_name)?;

        let payload_path = self.payload_path(id);
        let mut data = fs::read(&payload_path)?;
        data.reverse();
        fs::write(target, &data)?;

        fs::remove_file(&payload_path)?;
        fs::remove_file(self.vault.root().join(key_name))
    }

    fn payload_path(&self, id: &str) -> PathBuf {
        self.vault.root().join(format!("{id}.bin"))
    }

    fn key_name(id: &str) -> String {
        format!("{id}.key")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("secret.txt");
        fs::write(&source, b"confidential data").unwrap();

        let op = QuarantineOp::new(dir.path().join("vault")).unwrap();
        op.execute(&source, "tx123").unwrap();
        assert!(!source.exists());

        op.inverse(&source, "tx123").unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"confidential data");
    }
}
