use custodian_winsec::{apply_job_limits, spawn_in_appcontainer};
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Launches agent processes inside an AppContainer with job-object limits.
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

    /// Copies `original_exe` into the sandbox location and runs it there.
    pub fn spawn_agent(&self, original_exe: &Path, target_exe: &Path) -> Result<(), SandboxError> {
        fs::copy(original_exe, target_exe)?;
        spawn_in_appcontainer(&target_exe.to_string_lossy(), &[], &self.profile_name)?;
        // Placeholder: the real PID comes from CreateProcessAsUserW.
        apply_job_limits(0, self.memory_mb, self.cpu_rate)?;
        Ok(())
    }
}
