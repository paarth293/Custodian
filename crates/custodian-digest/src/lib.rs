//! Streaming BLAKE3 digests of files under a [`SafePath`].

use custodian_winsec::SafePath;
use std::fs::File;
use std::io::{self, Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigestError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Lowercase hex-encoded BLAKE3 digest.
pub type FileHash = String;

pub fn digest_file(path: &SafePath) -> Result<FileHash, DigestError> {
    let mut file = File::open(path.as_path())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 64 * 1024];

    loop {
        let count = file.read(&mut buffer)?;
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

    #[test]
    fn digests_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, b"hello world").unwrap();

        let safe_path = SafePath::new(&file_path, dir.path()).unwrap();
        assert_eq!(
            digest_file(&safe_path).unwrap(),
            "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
        );
    }
}
