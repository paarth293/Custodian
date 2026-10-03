use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct OffloadOp;

impl OffloadOp {
    pub fn execute(target: &SafePath, _upload_url: &str) -> io::Result<()> {
        let metadata = fs::metadata(target.as_path())?;
        fs::write(target.as_path(), format!("OFFLOADED: size={}", metadata.len()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_offload() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("large_file.bin");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(&[0u8; 1024]).unwrap();

        let safe_path = SafePath::new(&file_path, &temp_dir).unwrap();
        OffloadOp::execute(&safe_path, "http://mock-s3-url").unwrap();

        let data = fs::read_to_string(&file_path).unwrap();
        assert_eq!(data, "OFFLOADED: size=1024");
    }
}
