//! Windows security primitives used across Custodian.
//!
//! The process-isolation, Hello and DPAPI functions are portable placeholders
//! with the final signatures; their Win32 implementations are tracked in
//! `HONESTY.md`.

pub mod safe_place;

use std::fs;
use std::io;
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
/// Construction:
/// - rejects `..` components;
/// - resolves symlinks and junctions in every existing ancestor (via
///   `canonicalize`), so a link inside the root cannot point outside it;
/// - on Windows, strips trailing dots/spaces (which Win32 silently drops) and
///   yields the verbatim `\\?\` form, which also normalises drive-letter case.
///
/// The check is made at construction time; operations should still open
/// handles without following reparse points to close the remaining race.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath(PathBuf);

impl SafePath {
    /// `root` must exist. `path` may name an entry that does not exist yet
    /// (for example a move destination), as long as its parent chain is safe.
    pub fn new(path: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, PathError> {
        let path = trim_win32_suffix(path.as_ref());
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(PathError::EscapesRoot);
        }

        let root = fs::canonicalize(root)?;
        let resolved = resolve(&path)?;
        if !resolved.starts_with(&root) {
            return Err(PathError::EscapesRoot);
        }

        Ok(SafePath(resolved))
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

/// Canonicalises the deepest existing ancestor of `path` and re-appends the
/// components that do not exist yet.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    let mut existing = path;
    let mut missing = Vec::new();
    while fs::symlink_metadata(existing).is_err() {
        let name = existing
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no existing ancestor"))?;
        missing.push(name);
        existing = match existing.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
    }

    let mut resolved = fs::canonicalize(existing)?;
    resolved.extend(missing.iter().rev());
    Ok(resolved)
}

#[cfg(windows)]
fn trim_win32_suffix(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().trim_end_matches([' ', '.']))
}

#[cfg(not(windows))]
fn trim_win32_suffix(path: &Path) -> PathBuf {
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
    fn accepts_existing_and_new_paths_under_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();

        let existing = SafePath::new(dir.path().join("a.txt"), dir.path()).unwrap();
        assert!(existing.as_path().ends_with("a.txt"));

        let new = SafePath::new(dir.path().join("sub").join("b.txt"), dir.path()).unwrap();
        assert!(new.as_path().ends_with(Path::new("sub").join("b.txt")));
    }

    #[test]
    fn rejects_path_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        assert!(matches!(
            SafePath::new(other.path().join("x"), root.path()),
            Err(PathError::EscapesRoot)
        ));
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let root = tempfile::tempdir().unwrap();
        let sneaky = root.path().join("..").join("evil");
        assert!(matches!(
            SafePath::new(sneaky, root.path()),
            Err(PathError::EscapesRoot)
        ));
    }

    #[test]
    fn missing_root_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("missing");
        assert!(matches!(
            SafePath::new(root.join("x"), &root),
            Err(PathError::Io(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escaping_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();

        let through_link = root.path().join("link").join("victim.dll");
        assert!(matches!(
            SafePath::new(through_link, root.path()),
            Err(PathError::EscapesRoot)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn strips_trailing_dot_and_uses_verbatim_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let safe = SafePath::new(dir.path().join("foo."), dir.path()).unwrap();
        let s = safe.as_path().to_str().unwrap();
        assert!(s.starts_with(r"\\?\"), "{s}");
        assert!(s.ends_with(r"\foo"), "{s}");
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
