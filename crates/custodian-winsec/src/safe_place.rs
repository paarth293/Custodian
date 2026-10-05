use crate::{protect_data, unprotect_data};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A directory whose files are stored encrypted for the current user.
pub struct SafePlace {
    root: PathBuf,
}

impl SafePlace {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn write_encrypted(&self, filename: &str, data: &[u8]) -> io::Result<()> {
        fs::write(self.root.join(filename), protect_data(data)?)
    }

    pub fn read_decrypted(&self, filename: &str) -> io::Result<Vec<u8>> {
        unprotect_data(&fs::read(self.root.join(filename))?)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let place = SafePlace::new(dir.path().join("vault")).unwrap();

        let secret = b"my_super_secret_sidecar_data";
        place.write_encrypted("sidecar.bin", secret).unwrap();
        let on_disk = fs::read(place.root().join("sidecar.bin")).unwrap();
        assert!(!on_disk.windows(secret.len()).any(|w| w == secret));
        assert_eq!(place.read_decrypted("sidecar.bin").unwrap(), secret);
    }
}
