use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("Path escapes root")]
    EscapesRoot,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid path")]
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath(PathBuf);

impl SafePath {
    pub fn new<P: AsRef<Path>>(path: P, root: P) -> Result<Self, PathError> {
        let path = path.as_ref();
        let root_unc = format!(r"\\?\{}", root.as_ref().to_string_lossy());
        let root_path = PathBuf::from(root_unc);

        let path_str = path.to_string_lossy();
        let trimmed = path_str.trim_end_matches(&[' ', '.'][..]);
        
        let mut unc = String::from(r"\\?\");
        if !trimmed.starts_with(r"\\?\") {
            unc.push_str(&trimmed);
        } else {
            unc = trimmed.to_string();
        }

        let unc_path = PathBuf::from(unc);
        
        if !unc_path.starts_with(&root_path) {
            return Err(PathError::EscapesRoot);
        }

        Ok(SafePath(unc_path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trailing_dot() {
        let root = Path::new(r"C:\test_root");
        let path = Path::new(r"C:\test_root\foo.");
        let safe = SafePath::new(path, root).unwrap();
        assert_eq!(safe.as_path().to_str().unwrap(), r"\\?\C:\test_root\foo");
    }

    #[test]
    fn test_escape_root() {
        let root = Path::new(r"C:\test_root");
        let path = Path::new(r"D:\evil");
        assert!(matches!(SafePath::new(path, root), Err(PathError::EscapesRoot)));
    }
}
