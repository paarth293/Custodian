use std::path::{Component, Path, PathBuf};
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

/// Verbatim (extended-length) path prefix; only meaningful on Windows.
#[cfg(windows)]
const VERBATIM_PREFIX: &str = r"\\?\";

/// Converts a path string to its canonical on-disk form for this platform:
/// on Windows an extended-length `\\?\` path with trailing dots and spaces
/// stripped (Win32 would silently drop them), elsewhere the path unchanged.
#[cfg(windows)]
fn to_verbatim(raw: &str) -> String {
    let trimmed = raw.trim_end_matches([' ', '.']);
    if trimmed.starts_with(VERBATIM_PREFIX) {
        trimmed.to_string()
    } else {
        format!("{VERBATIM_PREFIX}{trimmed}")
    }
}

#[cfg(not(windows))]
fn to_verbatim(raw: &str) -> String {
    raw.to_string()
}

impl SafePath {
    /// Builds a path that is guaranteed to live under `root`.
    ///
    /// Any `..` component is rejected outright: a lexical `starts_with` check
    /// would otherwise accept `root\..\elsewhere`.
    pub fn new<P: AsRef<Path>>(path: P, root: P) -> Result<Self, PathError> {
        let path = path.as_ref();
        let root = root.as_ref();

        if [path, root]
            .iter()
            .any(|p| p.components().any(|c| matches!(c, Component::ParentDir)))
        {
            return Err(PathError::EscapesRoot);
        }

        let root_path = PathBuf::from(to_verbatim(&root.to_string_lossy()));
        let unc_path = PathBuf::from(to_verbatim(&path.to_string_lossy()));

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

    #[cfg(windows)]
    #[test]
    fn test_trailing_dot() {
        let root = Path::new(r"C:\test_root");
        let path = Path::new(r"C:\test_root\foo.");
        let safe = SafePath::new(path, root).unwrap();
        assert_eq!(safe.as_path().to_str().unwrap(), r"\\?\C:\test_root\foo");
    }

    #[cfg(windows)]
    #[test]
    fn test_escape_root() {
        let root = Path::new(r"C:\test_root");
        let path = Path::new(r"D:\evil");
        assert!(matches!(SafePath::new(path, root), Err(PathError::EscapesRoot)));
    }

    #[cfg(windows)]
    #[test]
    fn test_dotdot_rejected() {
        let root = Path::new(r"C:\test_root");
        let path = Path::new(r"C:\test_root\..\evil");
        assert!(matches!(SafePath::new(path, root), Err(PathError::EscapesRoot)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_escape_and_dotdot_rejected() {
        let root = Path::new("/test_root");
        assert!(matches!(
            SafePath::new(Path::new("/evil"), root),
            Err(PathError::EscapesRoot)
        ));
        assert!(matches!(
            SafePath::new(Path::new("/test_root/../evil"), root),
            Err(PathError::EscapesRoot)
        ));
        // A sibling that merely shares a string prefix must not pass.
        assert!(matches!(
            SafePath::new(Path::new("/test_root2/file"), root),
            Err(PathError::EscapesRoot)
        ));
        let ok = SafePath::new(Path::new("/test_root/sub/file."), root).unwrap();
        assert_eq!(ok.as_path(), Path::new("/test_root/sub/file."));
    }
}

use std::process::Command;

pub fn spawn_in_appcontainer(exe: &str, args: &[&str], profile_name: &str) -> std::io::Result<()> {
    // Mock: stands in for the unsafe Windows API calls (CreateAppContainerProfile, CreateProcessAsUserW)
    let _ = profile_name;
    
    let mut child = Command::new(exe).args(args).spawn()?;
    let _ = child.wait()?;
    Ok(())
}

#[cfg(test)]
mod test_appcontainer {
    use super::*;
    
    #[cfg(windows)]
    #[test]
    fn test_spawn() {
        let result = spawn_in_appcontainer("cmd.exe", &["/c", "exit 0"], "test_profile");
        assert!(result.is_ok());
    }

    #[cfg(not(windows))]
    #[test]
    fn test_spawn() {
        let result = spawn_in_appcontainer("true", &[], "test_profile");
        assert!(result.is_ok());
    }
}

pub fn apply_job_limits(pid: u32, memory_mb: u32, cpu_rate: u32) -> std::io::Result<()> {
    // Mock: stands in for CreateJobObjectW, SetInformationJobObject, AssignProcessToJobObject
    let _ = (pid, memory_mb, cpu_rate);
    Ok(())
}

#[cfg(test)]
mod test_job_limits {
    use super::*;
    
    #[test]
    fn test_apply_limits() {
        let result = apply_job_limits(1234, 512, 10);
        assert!(result.is_ok());
    }
}

pub fn prompt_biometrics(message: &str) -> bool {
    // Mock: stands in for LsaLogonUser / Windows Hello API
    let _ = message;
    true // Assume success for tests
}

#[cfg(test)]
mod test_hello {
    use super::*;
    
    #[test]
    fn test_prompt() {
        assert!(prompt_biometrics("Approve file deletion"));
    }
}

pub fn protect_data(plaintext: &[u8]) -> std::io::Result<Vec<u8>> {
    // Mock: stands in for CryptProtectData
    let _ = plaintext;
    // Mock encryption by reversing bytes
    Ok(plaintext.iter().rev().copied().collect())
}

pub fn unprotect_data(ciphertext: &[u8]) -> std::io::Result<Vec<u8>> {
    // Mock: stands in for CryptUnprotectData
    let _ = ciphertext;
    // Mock decryption by reversing bytes
    Ok(ciphertext.iter().rev().copied().collect())
}

#[cfg(test)]
mod test_dpapi {
    use super::*;
    
    #[test]
    fn test_dpapi_roundtrip() {
        let plaintext = b"secret-key-material";
        let encrypted = protect_data(plaintext).unwrap();
        let decrypted = unprotect_data(&encrypted).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }
}
pub mod safe_place;

