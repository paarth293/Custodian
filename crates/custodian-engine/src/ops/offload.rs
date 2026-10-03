use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct OffloadOp;

impl OffloadOp {
    /// Replaces `target` with a stub recording its original size.
    ///
    /// Placeholder: the upload to `upload_url` is not implemented yet.
    pub fn execute(target: &SafePath, upload_url: &str) -> io::Result<()> {
        let _ = upload_url;
        let len = fs::metadata(target.as_path())?.len();
        fs::write(target.as_path(), format!("OFFLOADED: size={len}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_stub() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large_file.bin");
        fs::write(&file_path, [0u8; 1024]).unwrap();

        OffloadOp::execute(
            &SafePath::new(&file_path, dir.path()).unwrap(),
            "https://example.invalid",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "OFFLOADED: size=1024"
        );
    }
}
