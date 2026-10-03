use custodian_winsec::SafePath;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};

const CHUNK: usize = 4096;

pub struct PermDeleteOp;

impl PermDeleteOp {
    /// Overwrites the file with zeros, flushes it to disk, then removes it.
    pub fn execute(target: &SafePath) -> io::Result<()> {
        let path = target.as_path();
        if !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }

        let mut remaining = fs::metadata(path)?.len();
        {
            let mut file = OpenOptions::new().write(true).open(path)?;
            let zeros = [0u8; CHUNK];
            while remaining > 0 {
                let n = remaining.min(CHUNK as u64) as usize;
                file.write_all(&zeros[..n])?;
                remaining -= n as u64;
            }
            file.sync_all()?;
        }

        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipes_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("wipe_test.txt");
        fs::write(&file_path, b"sensitive secret data").unwrap();

        PermDeleteOp::execute(&SafePath::new(&file_path, dir.path()).unwrap()).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn rejects_directories() {
        let dir = tempfile::tempdir().unwrap();
        assert!(PermDeleteOp::execute(&SafePath::new(dir.path(), dir.path()).unwrap()).is_err());
    }
}
