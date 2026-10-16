//! File moves that never overwrite anything.
//!
//! `std::fs::rename` replaces an existing target on Windows, so a check like
//! "the original location is free" followed by a rename can lose a file that
//! appears in between. These moves fail with `AlreadyExists` instead.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileIdentity {
    pub volume: u64,
    pub index: u64,
}

/// Security-only metadata query; no file contents are read.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn file_link_count(path: &Path) -> io::Result<u64> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_OPEN_REPARSE_POINT,
    };
    let file = OpenOptions::new()
        .access_mode(0x80)
        .share_mode(7)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    // SAFETY: numeric Win32 structure and a live file handle.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: file owns a live handle and info is writable for this call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(u64::from(info.nNumberOfLinks))
}

#[cfg(unix)]
pub fn file_link_count(path: &Path) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(fs::symlink_metadata(path)?.nlink())
}

/// Only fixed/removable local volumes are eligible for offline file mutations.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn is_local_volume(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return false;
    };
    let letter = match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
        _ => return false,
    };
    let root = [u16::from(letter), b':' as u16, b'\\' as u16, 0];
    // SAFETY: root is a live NUL-terminated UTF-16 drive-root string.
    matches!(unsafe { GetDriveTypeW(root.as_ptr()) }, 2 | 3)
}

#[cfg(not(windows))]
pub fn is_local_volume(path: &Path) -> bool {
    path.is_absolute()
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn identity_from_file(file: &File) -> io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    // SAFETY: the Win32 output structure contains only numeric fields and FILETIME values.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: file owns a valid handle, and info is writable for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & 0x400 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "reparse points are not supported",
        ));
    }
    Ok(FileIdentity {
        volume: info.dwVolumeSerialNumber as u64,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(windows)]
pub fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    identity_from_file(&file)
}

#[cfg(unix)]
pub fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symlinks are not supported",
        ));
    }
    Ok(FileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
    })
}

/// Verify and delete the same opened object; a path replacement cannot substitute another file.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn remove_verified(
    path: &Path,
    expected: FileIdentity,
    verify: impl FnOnce(&mut File) -> io::Result<bool>,
) -> io::Result<()> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const GENERIC_READ_ACCESS: u32 = 0x8000_0000;
    let mut file = OpenOptions::new()
        .access_mode(GENERIC_READ_ACCESS | DELETE_ACCESS)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    if identity_from_file(&file)? != expected || !verify(&mut file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file identity or content changed; nothing was deleted",
        ));
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: file owns the handle, and the pointer and size match FileDispositionInfo.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    drop(file);
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_verified(
    _path: &Path,
    _expected: FileIdentity,
    _verify: impl FnOnce(&mut File) -> io::Result<bool>,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "verified handle deletion is currently supported on Windows only",
    ))
}

/// Renames `from` to `to` on one volume, failing with `AlreadyExists` rather
/// than replacing `to`, and with `CrossesDevices` across volumes.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let (from_w, to_w) = (wide(from), wide(to));
    // SAFETY: both arguments are NUL-terminated UTF-16 buffers that outlive
    // the call. Without MOVEFILE_REPLACE_EXISTING the call never replaces a
    // file, and without MOVEFILE_COPY_ALLOWED it never copies.
    let ok = unsafe { MoveFileExW(from_w.as_ptr(), to_w.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Renames `from` to `to`, failing with `AlreadyExists` rather than replacing `to`.
#[cfg(not(windows))]
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

/// Moves a file without ever replacing `to`. On one volume this is a rename.
/// Across volumes the file is copied into a new file, checked with `verify`,
/// and only then removed from `from`. On any failure `from` stays where it
/// was and no partial copy is left behind.
pub fn move_no_replace(
    from: &Path,
    to: &Path,
    verify: impl FnOnce(&Path) -> io::Result<bool>,
) -> io::Result<()> {
    match rename_no_replace(from, to) {
        Ok(()) => {
            let result = verify(to).and_then(|valid| {
                if valid {
                    Ok(())
                } else {
                    Err(io::Error::other("move verification failed"))
                }
            });
            if result.is_err() {
                rename_no_replace(to, from).map_err(|rollback| {
                    io::Error::other(format!(
                        "move verification failed and rollback failed: {rollback}; payload remains at {}",
                        to.display()
                    ))
                })?;
            }
            result
        }
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            // `create_new` fails if `to` exists, so the rollback below only ever
            // removes a file this function created.
            let mut source = File::open(from)?;
            let mut out = OpenOptions::new().write(true).create_new(true).open(to)?;
            let copied = io::copy(&mut source, &mut out).and_then(|_| out.sync_all());
            drop(source);
            drop(out);
            let result = copied.and_then(|()| {
                if verify(to)? {
                    fs::remove_file(from)
                } else {
                    Err(io::Error::other("copy verification failed"))
                }
            });
            if result.is_err() {
                let _ = fs::remove_file(to);
            }
            result
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_within_a_volume() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"data").unwrap();
        move_no_replace(&a, &b, |_| Ok(true)).unwrap();
        assert!(!a.exists());
        assert_eq!(fs::read(&b).unwrap(), b"data");
    }

    #[test]
    fn moves_within_a_volume_rolls_back_on_failed_verification() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"payload").unwrap();
        let err = move_no_replace(&a, &b, |_| Ok(false)).unwrap_err();
        assert!(err.to_string().contains("verification"), "{err}");
        assert!(a.exists(), "source exists after rollback");
        assert!(!b.exists(), "destination cleaned up on rollback");
        assert_eq!(fs::read(&a).unwrap(), b"payload");
    }

    #[test]
    fn never_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"mine").unwrap();
        fs::write(&b, b"theirs").unwrap();
        let err = move_no_replace(&a, &b, |_| Ok(true)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&a).unwrap(), b"mine");
        assert_eq!(fs::read(&b).unwrap(), b"theirs");
    }

    #[test]
    fn verification_io_error_restores_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"payload").unwrap();
        let err = move_no_replace(&a, &b, |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot verify",
            ))
        })
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&a).unwrap(), b"payload");
        assert!(!b.exists());
    }

    #[test]
    fn a_missing_source_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(move_no_replace(&missing, &dir.path().join("b"), |_| Ok(true)).is_err());
        assert!(!dir.path().join("b").exists());
    }

    /// Needs a directory on another volume in `CUSTODIAN_SECOND_VOLUME`
    /// (skipped otherwise, as in CI).
    #[test]
    fn moves_across_volumes_with_verification() {
        let Some(other) = std::env::var_os("CUSTODIAN_SECOND_VOLUME") else {
            return;
        };
        let here = tempfile::tempdir().unwrap();
        let there = tempfile::tempdir_in(other).unwrap();
        let (a, b) = (here.path().join("a"), there.path().join("b"));

        fs::write(&a, b"payload").unwrap();
        let err = move_no_replace(&a, &b, |_| Ok(false)).unwrap_err();
        assert!(err.to_string().contains("verification"), "{err}");
        assert!(a.exists() && !b.exists(), "rolled back");

        move_no_replace(&a, &b, |p| Ok(fs::read(p)? == b"payload")).unwrap();
        assert!(!a.exists());
        assert_eq!(fs::read(&b).unwrap(), b"payload");

        fs::write(&a, b"new").unwrap();
        let err = move_no_replace(&a, &b, |_| Ok(true)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&b).unwrap(), b"payload");
    }
}
