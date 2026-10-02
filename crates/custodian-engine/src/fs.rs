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
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_execute_delete() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_delete.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test").unwrap();

        let safe_path = SafePath::new(&file_path, &temp_dir).unwrap();
        execute_delete(&safe_path).unwrap();

        assert!(!file_path.exists());
    }
}
