//! BLAKE3 hashing of files, streamed so large files use constant memory.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const BUFFER: usize = 64 * 1024;

/// Lowercase hex BLAKE3 digest of the whole file.
pub fn hash_file(path: &Path) -> io::Result<String> {
    hash_prefix(path, u64::MAX)
}

/// Digest of at most the first `limit` bytes; cheap pre-filter for large files.
pub fn hash_prefix(path: &Path, limit: u64) -> io::Result<String> {
    let mut reader = File::open(path)?.take(limit);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello world").unwrap();
        assert_eq!(
            hash_file(&path).unwrap(),
            "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
        );
    }

    #[test]
    fn prefix_ignores_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"same-start-AAA").unwrap();
        std::fs::write(&b, b"same-start-BBB").unwrap();
        assert_eq!(hash_prefix(&a, 10).unwrap(), hash_prefix(&b, 10).unwrap());
        assert_ne!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
    }
}
