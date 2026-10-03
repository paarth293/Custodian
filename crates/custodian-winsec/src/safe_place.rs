use crate::{protect_data, unprotect_data};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub struct SafePlace {
    root: PathBuf,
}

impl SafePlace {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn write_encrypted(&self, filename: &str, data: &[u8]) -> io::Result<()> {
        let encrypted = protect_data(data)?;
        let path = self.root.join(filename);
        let mut file = File::create(path)?;
        file.write_all(&encrypted)?;
        Ok(())
    }

    pub fn read_decrypted(&self, filename: &str) -> io::Result<Vec<u8>> {
        let path = self.root.join(filename);
        let mut file = File::open(path)?;
        let mut encrypted = Vec::new();
        file.read_to_end(&mut encrypted)?;
        unprotect_data(&encrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_place_roundtrip() {
        let temp_dir = std::env::temp_dir().join("safe_place_test");
        let safe_place = SafePlace::new(temp_dir).unwrap();
        
        let secret = b"my_super_secret_sidecar_data";
        safe_place.write_encrypted("sidecar.bin", secret).unwrap();
        
        let decrypted = safe_place.read_decrypted("sidecar.bin").unwrap();
        assert_eq!(secret.as_slice(), decrypted.as_slice());
    }
}
