use custodian_winsec::{spawn_in_appcontainer, apply_job_limits};
use std::path::Path;
use std::fs;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Spawn failed: {0}")]
    Spawn(String),
}

pub struct SandboxFactory {
    profile_name: String,
    memory_mb: u32,
    cpu_rate: u32,
}

impl SandboxFactory {
    pub fn new(profile_name: &str, memory_mb: u32, cpu_rate: u32) -> Self {
        Self {
            profile_name: profile_name.to_string(),
            memory_mb,
            cpu_rate,
        }
    }

    pub fn spawn_agent(&self, original_exe: &Path, target_exe: &Path) -> Result<(), SandboxError> {
        fs::copy(original_exe, target_exe)?;

        let exe_str = target_exe.to_string_lossy();
        spawn_in_appcontainer(&exe_str, &[], &self.profile_name)?;
        
        let fake_pid = 1234;
        apply_job_limits(fake_pid, self.memory_mb, self.cpu_rate)?;

        Ok(())
    }
}
