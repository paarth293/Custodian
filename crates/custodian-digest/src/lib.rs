use custodian_winsec::SafePath;
use std::fs::File;
use std::io::{self, BufReader, Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigestError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub type FileHash = String;

pub fn digest_file(path: &SafePath) -> Result<FileHash, DigestError> {
    let file = File::open(path.as_path())?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 8192];
    
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_digest_file() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_blake3.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();
        
        let safe_path = SafePath::new(&file_path, &temp_dir).unwrap();
        let hash = digest_file(&safe_path).unwrap();
        
        assert_eq!(hash, "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24");
    }
}
