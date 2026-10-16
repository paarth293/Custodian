use custodian_winsec::SafePath;
use std::io;

pub struct OffloadOp;

impl OffloadOp {
    /// Replaces `target` with a stub recording its original size.
    ///
    /// F09: Offload is disabled until durable upload, remote identity, and
    /// safe rollback are fully implemented. Returns an Unsupported error
    /// without modifying the file.
    pub fn execute(target: &SafePath, upload_url: &str) -> io::Result<()> {
        let _ = (target, upload_url);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "offload is not implemented: remote upload and rollback are unavailable",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn refuses_unsupported_offload_leaving_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large_file.bin");
        fs::write(&file_path, [0xAAu8; 1024]).unwrap();

        let result = OffloadOp::execute(
            &SafePath::new(&file_path, dir.path()).unwrap(),
            "https://example.invalid",
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Unsupported);
        assert_eq!(fs::read(&file_path).unwrap(), vec![0xAAu8; 1024]);
    }
}
