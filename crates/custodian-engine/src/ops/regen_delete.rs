use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct RegenDeleteOp;

impl RegenDeleteOp {
    pub fn execute(target: &SafePath) -> io::Result<()> {
        if target.as_path().is_file() {
            fs::remove_file(target.as_path())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "Target is not a file"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_regen_delete() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("regen_target.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"generated content").unwrap();

        let safe_path = SafePath::new(&file_path, &temp_dir).unwrap();
        RegenDeleteOp::execute(&safe_path).unwrap();

        assert!(!file_path.exists());
    }
}
