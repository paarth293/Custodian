use custodian_winsec::SafePath;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};

pub struct PermDeleteOp;

impl PermDeleteOp {
    pub fn execute(target: &SafePath) -> io::Result<()> {
        if !target.as_path().is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a file"));
        }
        
        let meta = fs::metadata(target.as_path())?;
        let len = meta.len();
        
        let mut file = OpenOptions::new().write(true).open(target.as_path())?;
        
        let zeros = vec![0u8; 4096];
        let mut written = 0;
        while written < len {
            let chunk = std::cmp::min(4096, len - written) as usize;
            file.write_all(&zeros[..chunk])?;
            written += chunk as u64;
        }
        file.sync_all()?;
        
        drop(file);
        fs::remove_file(target.as_path())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_perm_delete() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("wipe_test.txt");
        
        {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"sensitive secret data").unwrap();
        }

        let safe_path = SafePath::new(&file_path, &temp_dir).unwrap();
        PermDeleteOp::execute(&safe_path).unwrap();

        assert!(!file_path.exists());
    }
}
