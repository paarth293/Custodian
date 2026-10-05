use custodian_winsec::job::{Job, JobLimits};
use custodian_winsec::spawn_in_appcontainer;
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

    /// Copies `original_exe` into the sandbox location and runs it there,
    /// under a job with the factory's memory and CPU limits, until it exits.
    pub fn spawn_agent(&self, original_exe: &Path, target_exe: &Path) -> Result<(), SandboxError> {
        fs::copy(original_exe, target_exe)?;
        let job = Job::new(JobLimits {
            memory_mb: Some(self.memory_mb),
            cpu_percent: Some(self.cpu_rate),
            below_normal_priority: true,
        })?;
        let mut child =
            spawn_in_appcontainer(&target_exe.to_string_lossy(), &[], &self.profile_name)?;
        job.assign(&child)?;
        child.wait()?;
        Ok(())
    }
}
