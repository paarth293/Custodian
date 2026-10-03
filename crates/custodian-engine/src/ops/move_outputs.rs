use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct MoveOutputsOp;

impl MoveOutputsOp {
    pub fn execute(source: &SafePath, target: &SafePath) -> io::Result<()> {
        fs::rename(source.as_path(), target.as_path())
    }
    /// Undoes `execute` by moving the output back to where it came from.
    pub fn inverse(source: &SafePath, target: &SafePath) -> io::Result<()> {
        if source.as_path().exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Original location is occupied",
            ));
        }
        fs::rename(target.as_path(), source.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_and_inverse_preserve_data() {
        let dir = std::env::temp_dir().join("move_outputs_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let from = dir.join("out.txt");
        let to = dir.join("moved.txt");
        fs::write(&from, b"result").unwrap();

        let safe_from = SafePath::new(&from, &dir).unwrap();
        let safe_to = SafePath::new(&to, &dir).unwrap();
        MoveOutputsOp::execute(&safe_from, &safe_to).unwrap();
        assert!(!from.exists());

        MoveOutputsOp::inverse(&safe_from, &safe_to).unwrap();
        assert_eq!(fs::read(&from).unwrap(), b"result");
        assert!(!to.exists());
    }
}
