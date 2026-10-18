//! One authoritative desktop host per OS-user authority directory. The OS releases the
//! lock on process death; a stale PID file is never treated as authority.
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

#[derive(Debug)]
pub struct InstanceGuard {
    _file: File,
    #[cfg(windows)]
    _directories: Vec<File>,
}

impl InstanceGuard {
    pub fn acquire(data_dir: &Path) -> io::Result<Self> {
        Self::acquire_for_profile(data_dir, data_dir)
    }

    /// All desktop profiles use the same OS-user authority directory. A
    /// verification profile isolates state, not authority over local files.
    pub fn acquire_for_profile(authority_dir: &Path, data_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(authority_dir)?;
        fs::create_dir_all(data_dir)?;
        let directory = authority_dir.canonicalize()?;
        #[cfg(windows)]
        let state_directory = data_dir.canonicalize()?;
        #[cfg(windows)]
        let directories = {
            use std::os::windows::fs::OpenOptionsExt;
            let mut handles = Vec::new();
            for ancestor in directory
                .ancestors()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .chain(
                    state_directory
                        .ancestors()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev(),
                )
            {
                // FILE_READ_ATTRIBUTES, BACKUP_SEMANTICS, OPEN_REPARSE_POINT.
                // Pin the complete state path so it cannot be renamed and
                // replaced while this host still uses absolute state paths.
                let handle = OpenOptions::new()
                    .access_mode(0x80)
                    .share_mode(3)
                    .custom_flags(0x0200_0000 | 0x0020_0000)
                    .open(ancestor)?;
                use std::os::windows::fs::MetadataExt;
                if handle.metadata()?.file_attributes() & 0x400 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "State path was replaced by a reparse point",
                    ));
                }
                handles.push(handle);
            }
            handles
        };
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Permit another host to open the lock to discover it is held,
            // but prohibit unlink/rename replacement while our handle lives.
            options.share_mode(3).custom_flags(0x0020_0000);
        }
        let file = options.open(directory.join("host.lock"))?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if file.metadata()?.file_attributes() & 0x400 != 0
                || custodian_winsec::fsops::file_link_count(&directory.join("host.lock"))? != 1
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Host lock must be an ordinary single-linked file",
                ));
            }
        }
        file.try_lock().map_err(|_| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "Custodian is already running for this user, or its host lock is unavailable. Close the other instance before reopening.",
            )
        })?;
        Ok(Self {
            _file: file,
            #[cfg(windows)]
            _directories: directories,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    #[test]
    fn linked_lock_file_is_rejected_before_state_can_be_loaded() {
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join("other-file");
        fs::write(&other, b"do not touch").unwrap();
        fs::hard_link(&other, root.path().join("host.lock")).unwrap();
        assert_eq!(
            InstanceGuard::acquire(root.path()).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(other).unwrap(), b"do not touch");
    }

    #[test]
    fn alternate_profiles_share_one_authority_lease() {
        let root = tempfile::tempdir().unwrap();
        let authority = root.path().join("authority");
        let first = root.path().join("profile-one");
        let second = root.path().join("profile-two");
        let host = InstanceGuard::acquire_for_profile(&authority, &first).unwrap();
        assert!(InstanceGuard::acquire_for_profile(&authority, &second).is_err());
        drop(host);
        assert!(InstanceGuard::acquire_for_profile(&authority, &second).is_ok());
    }

    // Spawn the real test executable, not a thread or a simulated lock owner.
    #[test]
    fn lock_owner_child() {
        let Some(root) = std::env::var_os("CUSTODIAN_LOCK_TEST_CHILD") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let _guard = InstanceGuard::acquire(&root).unwrap();
        fs::write(root.join("owner-ready"), b"locked").unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn second_process_is_excluded_and_killed_owner_releases_authority() {
        let root = tempfile::tempdir().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "instance::tests::lock_owner_child",
                "--nocapture",
            ])
            .env("CUSTODIAN_LOCK_TEST_CHILD", root.path())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        // Always reap our exact child, including on an assertion failure.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !root.path().join("owner-ready").exists() {
                assert!(child.try_wait().unwrap().is_none(), "lock owner exited");
                assert!(Instant::now() < deadline, "lock owner did not become ready");
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                InstanceGuard::acquire(root.path()).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            assert!(InstanceGuard::acquire(&root.path().join(".")).is_err());
            #[cfg(windows)]
            assert!(
                fs::rename(root.path().join("host.lock"), root.path().join("old.lock")).is_err()
            );
            #[cfg(windows)]
            assert!(fs::rename(root.path(), root.path().with_extension("replacement")).is_err());
            // Independent fixture profiles do not share state or authority.
            let other = tempfile::tempdir().unwrap();
            assert!(InstanceGuard::acquire(other.path()).is_ok());
        }));
        child.kill().unwrap();
        child.wait().unwrap();
        result.unwrap();
        let guard = InstanceGuard::acquire(root.path()).unwrap();
        assert!(InstanceGuard::acquire(root.path()).is_err());
        drop(guard);
        assert!(InstanceGuard::acquire(root.path()).is_ok());
    }
}
