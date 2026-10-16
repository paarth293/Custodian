use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub fn execute_move(from: &SafePath, to: &SafePath) -> io::Result<()> {
    custodian_winsec::fsops::move_no_replace(from.as_path(), to.as_path(), |_| Ok(true))
}

pub fn execute_delete(target: &SafePath) -> io::Result<()> {
    if target.as_path().is_dir() {
        fs::remove_dir_all(target.as_path())
    } else {
        fs::remove_file(target.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_delete.txt");
        fs::write(&file_path, b"test").unwrap();

        execute_delete(&SafePath::new(&file_path, dir.path()).unwrap()).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a.txt");
        let to = dir.path().join("b.txt");
        fs::write(&from, b"x").unwrap();

        execute_move(
            &SafePath::new(&from, dir.path()).unwrap(),
            &SafePath::new(&to, dir.path()).unwrap(),
        )
        .unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read(&to).unwrap(), b"x");
    }

    #[test]
    fn execute_move_refuses_to_overwrite_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a.txt");
        let to = dir.path().join("b.txt");
        fs::write(&from, b"source").unwrap();
        fs::write(&to, b"destination").unwrap();

        let err = execute_move(
            &SafePath::new(&from, dir.path()).unwrap(),
            &SafePath::new(&to, dir.path()).unwrap(),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&from).unwrap(), b"source");
        assert_eq!(fs::read(&to).unwrap(), b"destination");
    }
}
