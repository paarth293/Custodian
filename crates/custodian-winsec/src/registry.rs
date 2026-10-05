//! Reads a few registry values Custodian needs, such as where cloud storage
//! keeps its folders. Read-only: nothing here writes to the registry.

/// A registry hive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

#[cfg(windows)]
pub use imp::{string_value, string_values, subkeys};

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::Hive;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegGetValueW, RegOpenKeyExW, HKEY,
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ,
        RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
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
}
