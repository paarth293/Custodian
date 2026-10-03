//! Windows security primitives used across Custodian.
//!
//! The process-isolation, Hello and DPAPI functions are portable placeholders
//! with the final signatures; their Win32 implementations are tracked in
//! `HONESTY.md`.

pub mod safe_place;

use std::path::{Component, Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("path escapes root")]
    EscapesRoot,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A path proven to live under a given root.
///
/// Construction rejects `..` components (which `Path::starts_with` would
/// otherwise accept, e.g. `C:\root\..\evil`). On Windows the path is also
/// converted to its verbatim (`\\?\`) form and trailing dots/spaces are
/// stripped, so Win32 name normalisation cannot change its meaning later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath(PathBuf);

impl SafePath {
    pub fn new(path: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, PathError> {
        let path = path.as_ref();
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(PathError::EscapesRoot);
        }

        let path = normalize(path);
        let root = normalize(root.as_ref());
        if !path.starts_with(&root) {
            return Err(PathError::EscapesRoot);
        }

        Ok(SafePath(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for SafePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

#[cfg(windows)]
fn normalize(path: &Path) -> PathBuf {
    const VERBATIM: &str = r"\\?\";
    let s = path.to_string_lossy();
    let trimmed = s.trim_end_matches([' ', '.']);
    if trimmed.starts_with(VERBATIM) {
        PathBuf::from(trimmed)
    } else {
        PathBuf::from(format!("{VERBATIM}{trimmed}"))
    }
}

#[cfg(not(windows))]
fn normalize(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Launches `exe` inside the named AppContainer profile and waits for it.
///
/// Placeholder: currently a plain child process. The Win32 implementation
/// uses `CreateAppContainerProfile` and `CreateProcessAsUserW`.
pub fn spawn_in_appcontainer(exe: &str, args: &[&str], profile_name: &str) -> std::io::Result<()> {
    let _ = profile_name;
    Command::new(exe).args(args).spawn()?.wait()?;
    Ok(())
}

/// Applies memory and CPU limits to `pid` through a job object.
///
/// Placeholder for `CreateJobObjectW` / `SetInformationJobObject` /
/// `AssignProcessToJobObject`.
pub fn apply_job_limits(pid: u32, memory_mb: u32, cpu_rate: u32) -> std::io::Result<()> {
    let _ = (pid, memory_mb, cpu_rate);
    Ok(())
}

/// Prompts the user for Windows Hello verification.
///
/// Placeholder: always succeeds until the Hello integration lands.
pub fn prompt_biometrics(message: &str) -> bool {
    let _ = message;
    true
}

/// Encrypts data for the current user.
///
/// Placeholder for `CryptProtectData`. The transform is reversible but
/// provides **no** confidentiality.
pub fn protect_data(plaintext: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(plaintext.iter().rev().copied().collect())
}

/// Inverse of [`protect_data`]. Placeholder for `CryptUnprotectData`.
pub fn unprotect_data(ciphertext: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(ciphertext.iter().rev().copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_path_under_root() {
        let root = std::env::temp_dir();
        let safe = SafePath::new(root.join("a").join("b.txt"), &root).unwrap();
        assert!(safe.as_path().ends_with("b.txt"));
    }

    #[test]
    fn rejects_path_outside_root() {
        let root = std::env::temp_dir().join("custodian_root");
        let outside = std::env::temp_dir().join("elsewhere");
        assert!(matches!(
            SafePath::new(outside, root),
            Err(PathError::EscapesRoot)
        ));
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let root = std::env::temp_dir().join("custodian_root");
        let sneaky = root.join("..").join("evil");
        assert!(matches!(
            SafePath::new(sneaky, &root),
            Err(PathError::EscapesRoot)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn strips_trailing_dot_and_uses_verbatim_prefix() {
        let safe = SafePath::new(r"C:\test_root\foo.", r"C:\test_root").unwrap();
        assert_eq!(safe.as_path().to_str().unwrap(), r"\\?\C:\test_root\foo");
    }

    #[test]
    fn spawn_runs_child_to_completion() {
        #[cfg(windows)]
        let result = spawn_in_appcontainer("cmd.exe", &["/c", "exit 0"], "test_profile");
        #[cfg(not(windows))]
        let result = spawn_in_appcontainer("true", &[], "test_profile");
        assert!(result.is_ok());
    }

    #[test]
    fn job_limits_accept_valid_input() {
        assert!(apply_job_limits(1234, 512, 10).is_ok());
    }

    #[test]
    fn biometrics_placeholder_succeeds() {
        assert!(prompt_biometrics("Approve file deletion"));
    }

    #[test]
    fn dpapi_roundtrip() {
        let plaintext = b"secret-key-material";
        let encrypted = protect_data(plaintext).unwrap();
        assert_eq!(unprotect_data(&encrypted).unwrap(), plaintext);
    }
}
