use custodian_winsec::SafePath;
use std::fs;
use std::io;

/// Deletes a regenerable artefact (build output, cache file).
pub struct RegenDeleteOp;

impl RegenDeleteOp {
    pub fn execute(target: &SafePath) -> io::Result<()> {
        if target.as_path().is_file() {
            fs::remove_file(target.as_path())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "target is not a file",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("regen_target.txt");
        fs::write(&file_path, b"generated content").unwrap();

        RegenDeleteOp::execute(&SafePath::new(&file_path, dir.path()).unwrap()).unwrap();
        assert!(!file_path.exists());
    }
}
