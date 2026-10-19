//! Replace a native-owned settings file in the same directory, with write-through.
use std::{io, path::Path};

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    if !from.is_absolute() || from.parent() != to.parent() || from == to {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid settings replacement",
        ));
    }
    let encode = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let source = encode(from);
    let destination = encode(to);
    if source[..source.len() - 1].contains(&0) || destination[..destination.len() - 1].contains(&0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid settings path",
        ));
    }
    // SAFETY: live nul-terminated paths; flags replace only the native-owned
    // destination. The caller has synced the new file before requesting commit.
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn replace(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows settings commit required",
    ))
}
