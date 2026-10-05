//! Job objects: resource limits for child processes (build plan step 18).
//!
//! Processes assigned to a [`Job`] share its memory cap, CPU cap and priority,
//! and are killed when the `Job` is dropped, so a crash of Custodian never
//! leaves an orphaned model process eating memory.

use std::io;
use std::process::Child;

/// Limits applied to every process in a job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobLimits {
    /// Total committed memory of all processes in the job.
    pub memory_mb: Option<u32>,
    /// Hard CPU cap, in percent of the whole machine (1–100).
    pub cpu_percent: Option<u32>,
    /// Run below normal priority so the desktop stays responsive.
    pub below_normal_priority: bool,
}

/// A job object. Dropping it ends every process assigned to it.
#[derive(Debug)]
pub struct Job {
    /// The job handle, kept as an integer so `Job` can move between threads.
    #[cfg_attr(not(windows), allow(dead_code))]
    handle: isize,
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::{Job, JobLimits};
    use std::io;
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectCpuRateControlInformation,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOB_OBJECT_LIMIT_PRIORITY_CLASS,
    };
    use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;

    fn check(ok: i32) -> io::Result<()> {
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn create(limits: &JobLimits) -> io::Result<Job> {
        // SAFETY: null attributes and a null name create an unnamed job with
        // default security; the returned handle is checked below.
        let handle: HANDLE = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // Owned from here, so an error below still closes the handle.
        let job = Job {
            handle: handle as isize,
        };

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Some(mb) = limits.memory_mb {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            info.JobMemoryLimit = mb as usize * 1024 * 1024;
        }
        if limits.below_normal_priority {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PRIORITY_CLASS;
            info.BasicLimitInformation.PriorityClass = BELOW_NORMAL_PRIORITY_CLASS;
        }
        // SAFETY: `info` is a fully initialised struct of the size passed,
        // matching the information class, and lives for the call.
        check(unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        })?;

        if let Some(percent) = limits.cpu_percent {
            let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                    | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                ..Default::default()
            };
            // The rate is in hundredths of a percent.
            cpu.Anonymous.CpuRate = percent.clamp(1, 100) * 100;
            // SAFETY: as above, for the CPU rate information class.
            check(unsafe {
                SetInformationJobObject(
                    handle,
                    JobObjectCpuRateControlInformation,
                    (&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                    size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
                )
            })?;
        }
        Ok(job)
    }

    pub fn assign(job: &Job, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are open for the duration of the call: the
        // job's until it is dropped, the child's while `child` is borrowed.
        check(unsafe { AssignProcessToJobObject(job.handle as HANDLE, child.as_raw_handle()) })
    }

    pub fn assign_raw(job: &Job, process: isize) -> io::Result<()> {
        // SAFETY: the job handle is open until it is dropped; the caller
        // keeps the process handle open for the call.
        check(unsafe { AssignProcessToJobObject(job.handle as HANDLE, process as HANDLE) })
    }

    pub fn close(job: &Job) {
        // SAFETY: the handle came from CreateJobObjectW and is closed exactly
        // once, from `Drop`.
        unsafe { CloseHandle(job.handle as HANDLE) };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Job, JobLimits};
    use std::io;
    use std::process::Child;

    pub fn create(_: &JobLimits) -> io::Result<Job> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "job objects are available only on Windows",
        ))
    }

    pub fn assign(_: &Job, _: &Child) -> io::Result<()> {
        Ok(())
    }

    pub fn assign_raw(_: &Job, _: isize) -> io::Result<()> {
        Ok(())
    }

    pub fn close(_: &Job) {}
}

impl Job {
    pub fn new(limits: JobLimits) -> io::Result<Self> {
        imp::create(&limits)
    }

    /// Puts `child` under this job's limits. Processes it starts afterwards
    /// are included automatically.
    pub fn assign(&self, child: &Child) -> io::Result<()> {
        imp::assign(self, child)
    }

    /// Puts the process with this open handle under the job's limits.
    pub fn assign_raw(&self, process: isize) -> io::Result<()> {
        imp::assign_raw(self, process)
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        imp::close(self);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn sleeper() -> Child {
        Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn wait_for_exit(child: &mut Child, limit: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn dropping_the_job_ends_its_processes() {
        let job = Job::new(JobLimits {
            memory_mb: Some(1024),
            cpu_percent: Some(50),
            below_normal_priority: true,
        })
        .unwrap();
        let mut child = sleeper();
        job.assign(&child).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(job);
        assert!(wait_for_exit(&mut child, Duration::from_secs(5)));
    }

    /// Node allocating and touching 512 MB, after a pause so the job is in
    /// place first.
    fn allocate_512_mb() -> Command {
        let mut cmd = Command::new("node");
        cmd.args([
            "-e",
            "setTimeout(() => { Buffer.alloc(512 * 1024 * 1024, 1); }, 300)",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        cmd
    }

    #[test]
    fn the_memory_cap_is_enforced() {
        // Control: without the cap the allocation succeeds.
        assert!(allocate_512_mb().status().unwrap().success());

        let job = Job::new(JobLimits {
            memory_mb: Some(256),
            ..Default::default()
        })
        .unwrap();
        let mut child = allocate_512_mb().spawn().unwrap();
        job.assign(&child).unwrap();
        assert!(!child.wait().unwrap().success(), "the cap should stop it");
    }
}
