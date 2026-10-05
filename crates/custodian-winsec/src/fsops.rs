//! File moves that never overwrite anything.
//!
//! `std::fs::rename` replaces an existing target on Windows, so a check like
//! "the original location is free" followed by a rename can lose a file that
//! appears in between. These moves fail with `AlreadyExists` instead.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

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
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {}
        other => return other,
    }
    // `create_new` fails if `to` exists, so the rollback below only ever
    // removes a file this function created.
    let mut out = OpenOptions::new().write(true).create_new(true).open(to)?;
    let copied = io::copy(&mut File::open(from)?, &mut out).and_then(|_| out.sync_all());
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
