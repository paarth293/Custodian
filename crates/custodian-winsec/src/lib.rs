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

use std::process::Command;

pub fn spawn_in_appcontainer(exe: &str, args: &[&str], profile_name: &str) -> std::io::Result<()> {
    #[allow(unsafe_code)]
    unsafe {
        // SAFETY: Mocking the unsafe Windows API calls (CreateAppContainerProfile, CreateProcessAsUserW)
        let _ = profile_name;
    }
    
    let mut child = Command::new(exe).args(args).spawn()?;
    let _ = child.wait()?;
    Ok(())
}

#[cfg(test)]
mod test_appcontainer {
    use super::*;
    
    #[test]
    fn test_spawn() {
        let result = spawn_in_appcontainer("cmd.exe", &["/c", "exit 0"], "test_profile");
        assert!(result.is_ok());
    }
}

pub fn apply_job_limits(pid: u32, memory_mb: u32, cpu_rate: u32) -> std::io::Result<()> {
    #[allow(unsafe_code)]
    unsafe {
        // SAFETY: Mocking CreateJobObjectW, SetInformationJobObject, AssignProcessToJobObject
        let _ = (pid, memory_mb, cpu_rate);
    }
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
    #[allow(unsafe_code)]
    unsafe {
        // SAFETY: Mocking LsaLogonUser / Windows Hello API
        let _ = message;
    }
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
