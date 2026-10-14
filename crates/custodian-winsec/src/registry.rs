//! Reads a few registry values Custodian needs, such as where cloud storage
//! keeps its folders, and writes the per-user settings the person asked
//! Cubby to change (§settings).
//!
//! Writes exist only for the named settings in [`crate::settings`]: this
//! module takes a key path and a value name, and nothing above it may build
//! those from a model's words. Only `HKEY_CURRENT_USER` settings are changed;
//! nothing here touches another user or the machine as a whole.

/// A registry hive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

#[cfg(windows)]
pub use imp::{
    dword_value, remove_key, remove_value, set_dword, string_value, string_values, subkeys,
};

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::Hive;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteKeyValueW, RegEnumKeyExW, RegEnumValueW, RegGetValueW, RegOpenKeyExW,
        RegSetKeyValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD,
        REG_EXPAND_SZ, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };

    /// Longest key name the registry allows, plus its terminating NUL.
    const MAX_KEY_NAME: usize = 256;
    /// Longest value name the registry allows, plus its terminating NUL.
    const MAX_VALUE_NAME: usize = 16_384;
    /// Values longer than this (in bytes) are skipped; folder paths are far shorter.
    const MAX_VALUE_BYTES: usize = 64 * 1024;

    fn root(hive: Hive) -> HKEY {
        match hive {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text).encode_wide().chain(Some(0)).collect()
    }

    /// UTF-16 text up to the first NUL.
    fn text(units: &[u16]) -> String {
        let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
        String::from_utf16_lossy(&units[..end])
    }

    /// An open registry key, closed when dropped.
    struct Key(HKEY);

    impl Key {
        fn open(hive: Hive, path: &str) -> Option<Self> {
            let path = wide(path);
            let mut key: HKEY = null_mut();
            // SAFETY: `path` is a NUL-terminated UTF-16 string that outlives
            // the call, and `key` is a valid place for the opened handle.
            let status = unsafe { RegOpenKeyExW(root(hive), path.as_ptr(), 0, KEY_READ, &mut key) };
            (status == ERROR_SUCCESS).then_some(Self(key))
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful RegOpenKeyExW and is
            // closed exactly once, here.
            unsafe { RegCloseKey(self.0) };
        }
    }

    /// The names of the subkeys of `path`, or none if it cannot be opened.
    pub fn subkeys(hive: Hive, path: &str) -> Vec<String> {
        let Some(key) = Key::open(hive, path) else {
            return Vec::new();
        };
        let mut names = Vec::new();
        let mut buf = vec![0u16; MAX_KEY_NAME];
        for index in 0u32.. {
            let mut len = buf.len() as u32;
            // SAFETY: `buf` is writable for `len` UTF-16 units; the class and
            // time outputs are not requested (null), as the API allows.
            let status = unsafe {
                RegEnumKeyExW(
                    key.0,
                    index,
                    buf.as_mut_ptr(),
                    &mut len,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if status != ERROR_SUCCESS {
                break; // ERROR_NO_MORE_ITEMS: the end of the list.
            }
            names.push(text(&buf[..(len as usize).min(buf.len())]));
        }
        names
    }

    /// A string value (REG_SZ, or REG_EXPAND_SZ with its variables expanded).
    pub fn string_value(hive: Hive, path: &str, name: &str) -> Option<String> {
        let (path, name) = (wide(path), wide(name));
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
        let mut bytes: u32 = 0;
        // SAFETY: both strings are NUL-terminated and outlive the call; a null
        // data pointer asks only for the size, written to `bytes`.
        let status = unsafe {
            RegGetValueW(
                root(hive),
                path.as_ptr(),
                name.as_ptr(),
                flags,
                null_mut(),
                null_mut(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        // Expanding variables can make the value longer than first reported.
        for _ in 0..3 {
            let capacity = (bytes as usize).clamp(2, MAX_VALUE_BYTES);
            let mut buf = vec![0u16; capacity.div_ceil(2)];
            let mut size = (buf.len() * 2) as u32;
            // SAFETY: as above, and `buf` is writable for `size` bytes.
            let status = unsafe {
                RegGetValueW(
                    root(hive),
                    path.as_ptr(),
                    name.as_ptr(),
                    flags,
                    null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            match status {
                ERROR_SUCCESS => {
                    let units = (size as usize / 2).min(buf.len());
                    return Some(text(&buf[..units]));
                }
                ERROR_MORE_DATA if (size as usize) <= MAX_VALUE_BYTES => bytes = size,
                _ => return None,
            }
        }
        None
    }

    /// A 32-bit number stored at `path`\`name`, or `None` when the key,
    /// the value or the type is not there.
    pub fn dword_value(hive: Hive, path: &str, name: &str) -> Option<u32> {
        let (path, name) = (wide(path), wide(name));
        let mut value: u32 = 0;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        // SAFETY: both strings are NUL-terminated and outlive the call;
        // `value` is writable for the `size` bytes passed in.
        let status = unsafe {
            RegGetValueW(
                root(hive),
                path.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                null_mut(),
                std::ptr::addr_of_mut!(value).cast(),
                &mut size,
            )
        };
        (status == ERROR_SUCCESS && size == std::mem::size_of::<u32>() as u32).then_some(value)
    }

    /// Stores `value` at `path`\`name`, creating the key when it is missing.
    pub fn set_dword(hive: Hive, path: &str, name: &str, value: u32) -> std::io::Result<()> {
        let (path, name) = (wide(path), wide(name));
        let bytes = value.to_le_bytes();
        // SAFETY: both strings are NUL-terminated and outlive the call; the
        // data pointer is valid for the 4 bytes passed as its size.
        let status = unsafe {
            RegSetKeyValueW(
                root(hive),
                path.as_ptr(),
                name.as_ptr(),
                REG_DWORD,
                bytes.as_ptr().cast(),
                bytes.len() as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(status as i32))
        }
    }

    /// Deletes the key at `path`, which must have no subkeys.
    pub fn remove_key(hive: Hive, path: &str) -> std::io::Result<()> {
        use windows_sys::Win32::System::Registry::RegDeleteKeyW;
        let path = wide(path);
        // SAFETY: `path` is a NUL-terminated string that outlives the call.
        let status = unsafe { RegDeleteKeyW(root(hive), path.as_ptr()) };
        // A key that is already gone is the state the caller asked for.
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(status as i32))
        }
    }

    /// Removes the value at `path`\`name`. The key is left in place, so
    /// setting it back to what it was is a matter of writing it again.
    pub fn remove_value(hive: Hive, path: &str, name: &str) -> std::io::Result<()> {
        let (path, name) = (wide(path), wide(name));
        // SAFETY: both strings are NUL-terminated and outlive the call.
        let status = unsafe { RegDeleteKeyValueW(root(hive), path.as_ptr(), name.as_ptr()) };
        // A value that is already gone is the state the caller asked for.
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(status as i32))
        }
    }

    /// Every string value of the key at `path`, as (name, data). Values are
    /// returned as stored, without expanding variables.
    pub fn string_values(hive: Hive, path: &str) -> Vec<(String, String)> {
        let Some(key) = Key::open(hive, path) else {
            return Vec::new();
        };
        let mut values = Vec::new();
        let mut name = vec![0u16; MAX_VALUE_NAME];
        let mut data = vec![0u8; MAX_VALUE_BYTES];
        for index in 0u32.. {
            let mut name_len = name.len() as u32;
            let mut data_len = data.len() as u32;
            let mut kind: u32 = 0;
            // SAFETY: `name` is writable for `name_len` UTF-16 units and
            // `data` for `data_len` bytes; the reserved argument is null.
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    null(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut data_len,
                )
            };
            if status == ERROR_MORE_DATA {
                continue; // Longer than any folder path: skip this value.
            }
            if status != ERROR_SUCCESS {
                break; // ERROR_NO_MORE_ITEMS: the end of the list.
            }
            if kind != REG_SZ && kind != REG_EXPAND_SZ {
                continue;
            }
            let bytes = &data[..(data_len as usize).min(data.len())];
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            values.push((
                text(&name[..(name_len as usize).min(name.len())]),
                text(&units),
            ));
        }
        values
    }
}

#[cfg(not(windows))]
pub fn subkeys(_hive: Hive, _path: &str) -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn string_value(_hive: Hive, _path: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn string_values(_hive: Hive, _path: &str) -> Vec<(String, String)> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn dword_value(_hive: Hive, _path: &str, _name: &str) -> Option<u32> {
    None
}

#[cfg(not(windows))]
pub fn set_dword(_hive: Hive, _path: &str, _name: &str, _value: u32) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the registry is available only on Windows",
    ))
}

#[cfg(not(windows))]
pub fn remove_value(_hive: Hive, _path: &str, _name: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the registry is available only on Windows",
    ))
}

#[cfg(not(windows))]
pub fn remove_key(_hive: Hive, _path: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the registry is available only on Windows",
    ))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    #[test]
    fn reads_values_that_every_windows_pc_has() {
        let product = string_value(Hive::LocalMachine, CURRENT_VERSION, "ProductName").unwrap();
        assert!(product.contains("Windows"), "{product}");
        let root = string_value(Hive::LocalMachine, CURRENT_VERSION, "SystemRoot").unwrap();
        assert!(root.to_ascii_lowercase().ends_with("windows"), "{root}");
    }

    #[test]
    fn lists_subkeys_and_values() {
        let keys = subkeys(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT");
        assert!(keys.iter().any(|k| k == "CurrentVersion"), "{keys:?}");
        let values = string_values(Hive::LocalMachine, CURRENT_VERSION);
        assert!(values.iter().any(|(name, _)| name == "ProductName"));
    }

    #[test]
    fn missing_keys_and_values_are_simply_absent() {
        assert!(subkeys(Hive::CurrentUser, r"Software\Custodian\NoSuchKey\Here").is_empty());
        assert!(string_values(Hive::CurrentUser, r"Software\Custodian\NoSuchKey").is_empty());
        assert_eq!(
            string_value(Hive::LocalMachine, CURRENT_VERSION, "NoSuchValueAnywhere"),
            None
        );
    }

    /// The write path, exercised under a scratch key of its own so no real
    /// setting is read or changed, and nothing is left behind.
    #[test]
    fn writes_reads_and_removes_a_scratch_value() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let (parent, leaf) = (
            r"Software\Custodian\Test".to_string(),
            format!("Registry{}_{stamp}", std::process::id()),
        );
        let path = format!("{parent}\\{leaf}");
        let name = "Probe";
        let hive = Hive::CurrentUser;

        // Nothing there to begin with.
        assert_eq!(dword_value(hive, &path, name), None);
        remove_value(hive, &path, name).unwrap(); // Removing nothing is fine.

        set_dword(hive, &path, name, 0xDEAD_BEEF).unwrap();
        assert_eq!(dword_value(hive, &path, name), Some(0xDEAD_BEEF));
        set_dword(hive, &path, name, 0).unwrap();
        assert_eq!(dword_value(hive, &path, name), Some(0));

        remove_value(hive, &path, name).unwrap();
        assert_eq!(dword_value(hive, &path, name), None);
        remove_value(hive, &path, name).unwrap(); // And again.

        // Take the scratch keys away, innermost first. A container left by
        // another test is not ours to remove, so only the leaf must go.
        remove_key(hive, &path).unwrap();
        let _ = remove_key(hive, &parent);
        let _ = remove_key(hive, r"Software\Custodian");
    }
}
