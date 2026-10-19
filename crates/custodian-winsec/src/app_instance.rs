//! Metadata-only binding for a native-launched application instance. No titles.
#[cfg(windows)]
pub use native::{file_version, is_descendant, process_ids, WindowLease};

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use std::os::windows::ffi::OsStrExt;
    use std::{io, path::Path, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND, INVALID_HANDLE_VALUE, LPARAM},
        Storage::FileSystem::{
            GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
        },
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetPropW, GetWindowLongW, GetWindowThreadProcessId,
            IsWindowVisible, RemovePropW, SetPropW, GWL_STYLE, WS_DISABLED,
        },
    };
    fn invalid() -> io::Error {
        io::Error::other("Application identity could not be verified")
    }

    /// Process IDs only, to refuse attaching COM automation to an instance that
    /// existed before this task. No executable names, titles or content.
    pub fn process_ids() -> io::Result<Vec<u32>> {
        // SAFETY: metadata-only system snapshot.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(invalid());
        }
        let result = (|| {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut result = vec![];
            // SAFETY: initialized entry and live snapshot.
            let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
            if ok == 0 {
                return Err(invalid());
            }
            while ok != 0 {
                result.push(entry.th32ProcessID);
                if result.len() > 65536 {
                    return Err(invalid());
                }
                // SAFETY: same initialized entry and snapshot.
                ok = unsafe { Process32NextW(snapshot, &mut entry) };
            }
            Ok(result)
        })();
        // SAFETY: close only the snapshot created here.
        unsafe {
            CloseHandle(snapshot);
        }
        result
    }

    pub fn file_version(path: &Path) -> io::Result<[u16; 4]> {
        let path = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        if path[..path.len() - 1].contains(&0) {
            return Err(invalid());
        }
        let mut unused = 0;
        // SAFETY: live nul-terminated path and valid size output.
        let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), &mut unused) };
        if size == 0 || size > 1024 * 1024 {
            return Err(invalid());
        }
        let mut bytes = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: writable aligned storage of the exact size requested by Windows.
        if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, bytes.as_mut_ptr().cast()) } == 0 {
            return Err(invalid());
        }
        let mut value = ptr::null_mut();
        let mut length = 0;
        let root = [b'\\' as u16, 0];
        // SAFETY: initialized version block and live output pointers.
        if unsafe {
            VerQueryValueW(
                bytes.as_ptr().cast(),
                root.as_ptr(),
                &mut value,
                &mut length,
            )
        } == 0
            || length < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
            || value.is_null()
        {
            return Err(invalid());
        }
        // SAFETY: Windows returned an aligned fixed-file info structure of checked size.
        let info = unsafe { &*value.cast::<VS_FIXEDFILEINFO>() };
        if info.dwSignature != 0xfeef04bd {
            return Err(invalid());
        }
        Ok([
            (info.dwFileVersionMS >> 16) as u16,
            info.dwFileVersionMS as u16,
            (info.dwFileVersionLS >> 16) as u16,
            info.dwFileVersionLS as u16,
        ])
    }

    struct Search {
        pid: u32,
        handles: Vec<isize>,
    }
    unsafe extern "system" fn enumerate(hwnd: HWND, parameter: LPARAM) -> i32 {
        // SAFETY: EnumWindows receives a live Search pointer for this call only.
        let search = unsafe { &mut *(parameter as *mut Search) };
        let mut pid = 0;
        // SAFETY: metadata query with writable PID output; no title/content access.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        // SAFETY: a window visibility metadata query is valid for any HWND.
        if pid != search.pid || unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut class = [0u16; 128];
        // SAFETY: writable class-name buffer, only after exact PID filtering.
        let length = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), 128) };
        let name = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        if matches!(name.as_str(), "Chrome_WidgetWin_1" | "XLMAIN" | "OpusApp") {
            search.handles.push(hwnd as isize);
        }
        1
    }

    pub struct WindowLease {
        pid: u32,
        birth: u64,
        handle: isize,
        token: String,
        property: Vec<u16>,
    }
    impl WindowLease {
        pub fn admit(pid: u32, birth: u64, token: &str) -> io::Result<Self> {
            if token.len() != 36
                || !token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
                || !crate::apps::process_is_running(pid, birth)?
            {
                return Err(invalid());
            }
            let mut search = Search {
                pid,
                handles: Vec::new(),
            };
            // SAFETY: callback accesses only this live Search during synchronous enumeration.
            if unsafe { EnumWindows(Some(enumerate), &mut search as *mut Search as LPARAM) } == 0
                || search.handles.len() != 1
            {
                return Err(invalid());
            }
            Self::admit_handle(pid, birth, search.handles[0] as i64, token)
        }
        /// Bind a COM-returned window, including the dedicated hidden Excel
        /// window. Query class only after its PID/generation matches.
        pub fn admit_handle(pid: u32, birth: u64, handle: i64, token: &str) -> io::Result<Self> {
            if token.len() != 36
                || !token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
                || !crate::apps::process_is_running(pid, birth)?
            {
                return Err(invalid());
            }
            let mut measured_pid = 0;
            // SAFETY: metadata queries on a handle; no content or title access.
            unsafe {
                GetWindowThreadProcessId(handle as HWND, &mut measured_pid);
            }
            if measured_pid != pid {
                return Err(invalid());
            }
            let mut class = [0u16; 128];
            // SAFETY: exact PID was checked before class metadata.
            let length = unsafe { GetClassNameW(handle as HWND, class.as_mut_ptr(), 128) };
            let name = String::from_utf16_lossy(&class[..length.max(0) as usize]);
            if !matches!(name.as_str(), "Chrome_WidgetWin_1" | "XLMAIN" | "OpusApp") {
                return Err(invalid());
            }
            let property = format!("Custodian-Window-{token}")
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let handle = handle as isize;
            // SAFETY: metadata tag on the newly owned instance's one admitted window.
            // Window destruction discards this property, detecting HWND reuse.
            if unsafe { SetPropW(handle as HWND, property.as_ptr(), 1usize as _) } == 0 {
                return Err(invalid());
            }
            let lease = Self {
                pid,
                birth,
                handle,
                token: token.into(),
                property,
            };
            if !lease.current() {
                return Err(invalid());
            }
            Ok(lease)
        }
        pub fn current(&self) -> bool {
            Self::recognizes(self.pid, self.birth, self.handle as i64, &self.token)
                // SAFETY: metadata only on the exact generation-tagged HWND.
                && unsafe { GetWindowLongW(self.handle as HWND, GWL_STYLE) } as u32 & WS_DISABLED == 0
        }
        /// Read-only validation of a retained native window generation. Does
        /// not add a property, rebind a reused HWND or grant window input.
        pub fn recognizes(pid_expected: u32, birth: u64, handle: i64, token: &str) -> bool {
            if token.len() != 36 || !token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
                return false;
            }
            let property = format!("Custodian-Window-{token}")
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let mut pid = 0;
            // SAFETY: metadata queries only on the retained HWND token.
            unsafe {
                GetWindowThreadProcessId(handle as HWND, &mut pid);
            }
            // SAFETY: live nul-terminated property name; destroyed/reused windows lack it.
            pid == pid_expected
                && unsafe { GetPropW(handle as HWND, property.as_ptr()) } as usize == 1
                && crate::apps::process_is_running(pid_expected, birth).unwrap_or(false)
        }
        pub fn handle(&self) -> i64 {
            self.handle as i64
        }
        pub fn token(&self) -> &str {
            &self.token
        }
    }
    impl Drop for WindowLease {
        fn drop(&mut self) {
            if self.current() {
                // SAFETY: remove only our random property from the same live instance.
                unsafe {
                    RemovePropW(self.handle as HWND, self.property.as_ptr());
                }
            }
        }
    }

    pub fn is_descendant(child: u32, root: u32, root_birth: u64) -> bool {
        let Some(root_identity) = crate::apps::process_identity(root) else {
            return false;
        };
        if root_identity.creation_time != root_birth {
            return false;
        }
        // SAFETY: process metadata snapshot, no inheritable handles or content access.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut parents = std::collections::HashMap::new();
        // SAFETY: initialized PROCESSENTRY32W with its exact size.
        let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
        while ok != 0 {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            if parents.len() > 65536 {
                break;
            }
            // SAFETY: live snapshot and writable entry of the correct size.
            ok = unsafe { Process32NextW(snapshot, &mut entry) };
        }
        // SAFETY: close exactly our owned snapshot.
        unsafe {
            CloseHandle(snapshot);
        }
        let mut current = child;
        let mut later = u64::MAX;
        for _ in 0..64 {
            let Some(identity) = crate::apps::process_identity(current) else {
                return false;
            };
            if identity.creation_time < root_birth || identity.creation_time > later {
                return false;
            }
            if current == root {
                return identity.creation_time == root_birth;
            }
            later = identity.creation_time;
            let Some(parent) = parents.get(&current) else {
                return false;
            };
            current = *parent;
        }
        false
    }
}
