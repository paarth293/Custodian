use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub fn execute_move(from: &SafePath, to: &SafePath) -> io::Result<()> {
    fs::rename(from.as_path(), to.as_path())
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
}
