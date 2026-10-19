//! The windows the person has open on the desktop, and closing them politely.
//!
//! Closing sends a window the same request as clicking its X button
//! (`WM_CLOSE`), so an app with unsaved work can still ask to save it.
//! Nothing here ever force-quits a program.

/// A top-level window the person can see on the desktop or in the taskbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppWindow {
    /// The window handle, as a number.
    pub handle: isize,
    pub pid: u32,
    /// Process creation time token; a PID is not stable across process reuse.
    pub process_generation: Option<u64>,
    pub title: String,
    /// The program's file name, such as "notepad.exe" (empty if unknown).
    pub exe: String,
}

/// Minimal native identity probe. Contains no title, UIA text or document data.
/// The executable and creation time are queried through the same open handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub creation_time: u64,
    pub executable: String,
}

/// Window classes of the Windows shell itself (the desktop and the taskbar).
/// Asking them to close would start a shutdown, so they are never listed.
pub const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

#[cfg(windows)]
pub use imp::{
    is_open, open_windows, probe_owned_window, process_generation, process_identity,
    process_is_running, request_close,
};

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::{AppWindow, ProcessIdentity, SHELL_CLASSES};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindow, GetWindowLongW, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW,
        GWL_EXSTYLE, GW_OWNER, WM_CLOSE, WS_EX_TOOLWINDOW,
    };

    /// Adds each window EnumWindows reports to the Vec at `lparam`.
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the address of the Vec passed to EnumWindows in
        // `top_level_windows`, which outlives the enumeration and is not
        // otherwise touched while it runs.
        let found = unsafe { &mut *(lparam as *mut Vec<HWND>) };
        found.push(hwnd);
        1 // Keep going.
    }

    /// Every top-level window, frontmost first.
    fn top_level_windows() -> Vec<HWND> {
        let mut found: Vec<HWND> = Vec::new();
        // SAFETY: `collect` only pushes into `found`, which lives until
        // EnumWindows returns.
        unsafe { EnumWindows(Some(collect), &mut found as *mut Vec<HWND> as LPARAM) };
        found
    }

    fn class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 256];
        // SAFETY: `buf` is writable for the length passed.
        let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..(len.max(0) as usize).min(buf.len())])
    }

    fn title(hwnd: HWND) -> String {
        // SAFETY: a plain query on a window handle.
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        // SAFETY: `buf` is writable for the length passed.
        let copied = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..(copied.max(0) as usize).min(buf.len())])
    }

    /// Hidden by the system although "visible" (e.g. suspended Store apps).
    fn cloaked(hwnd: HWND) -> bool {
        let mut value: u32 = 0;
        // SAFETY: `value` is a writable u32 and its exact size is passed.
        let result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                (&mut value as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
        result == 0 && value != 0
    }

    /// The file name of process `pid`'s program, or "" if it cannot be read
    /// (for example a program running as administrator).
    fn exe_of(pid: u32) -> String {
        // SAFETY: a query-only open of a process; the handle is closed below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return String::new();
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` is writable for `len` UTF-16 units.
        let ok = unsafe {
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
        };
        // SAFETY: `process` came from OpenProcess and is closed exactly once.
        unsafe { CloseHandle(process) };
        if ok == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())]);
        full.rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_string()
    }

    /// Returns the process creation timestamp as a stable generation token.
    /// PIDs can be reused after a process exits, so a PID alone is never a
    /// sufficient application identity for an adapter.
    pub fn process_generation(pid: u32) -> Option<u64> {
        // SAFETY: a query-only open of a process; the handle is closed below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        // SAFETY: all FILETIME values are writable and the process handle is
        // valid for the query performed above.
        let ok =
            unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
        // SAFETY: `process` came from OpenProcess and is closed exactly once.
        unsafe { CloseHandle(process) };
        (ok != 0)
            .then(|| (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    /// Read metadata for one identified process only; never enumerate titles.
    /// This is a measurement, not an app/document grant or a retained lease.
    pub fn process_identity(pid: u32) -> Option<ProcessIdentity> {
        if pid == 0 {
            return None;
        }
        // SAFETY: query-only access to the supplied PID, closed on every path.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        let result = (|| {
            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = creation;
            let mut kernel = creation;
            let mut user = creation;
            // SAFETY: writable FILETIME values and a live owned query handle.
            if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
                == 0
            {
                return None;
            }
            let mut path = vec![0u16; 32_768];
            let mut length = path.len() as u32;
            // SAFETY: writable buffer, correct capacity and the same owned handle.
            if unsafe {
                QueryFullProcessImageNameW(
                    process,
                    PROCESS_NAME_WIN32,
                    path.as_mut_ptr(),
                    &mut length,
                )
            } == 0
            {
                return None;
            }
            Some(ProcessIdentity {
                pid,
                creation_time: (u64::from(creation.dwHighDateTime) << 32)
                    | u64::from(creation.dwLowDateTime),
                executable: String::from_utf16(&path[..length as usize]).ok()?,
            })
        })();
        // SAFETY: close the query handle exactly once, even on measurement failure.
        unsafe { CloseHandle(process) };
        result
    }

    /// Creates and destroys an empty message-only window owned by this process.
    /// No user window is enumerated/read, no pixels are captured, no UI is shown.
    pub fn probe_owned_window() -> std::io::Result<()> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, HWND_MESSAGE,
        };
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title = [0u16];
        // SAFETY: built-in system class, valid nul-terminated text, no user data,
        // no visible style, and the message-only parent creates no desktop UI.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut pid = 0;
        // SAFETY: query our own newly created handle, with writable PID storage.
        let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let valid = thread != 0 && pid == std::process::id();
        // SAFETY: destroy exactly our own window on its creating thread.
        let destroyed = unsafe { DestroyWindow(hwnd) } != 0;
        if !valid || !destroyed {
            return Err(std::io::Error::other("owned window probe failed"));
        }
        Ok(())
    }

    /// Check termination on a native process handle, bound to its creation time.
    /// A retained exited process object can still have readable image metadata.
    pub fn process_is_running(pid: u32, expected_creation: u64) -> std::io::Result<bool> {
        use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        if pid == 0 {
            return Ok(false);
        }
        const SYNCHRONIZE: u32 = 0x00100000;
        // SAFETY: minimal query/wait access; the returned owned handle is closed.
        let process =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid) };
        if process.is_null() {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(87) {
                Ok(false)
            } else {
                Err(error)
            };
        }
        let result = (|| {
            let mut creation = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = creation;
            let mut kernel = creation;
            let mut user = creation;
            // SAFETY: same owned query handle and writable FILETIME outputs.
            if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
                == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            let actual =
                (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
            if actual != expected_creation {
                return Ok(false);
            }
            // SAFETY: zero-time wait on the same owned process handle.
            match unsafe { WaitForSingleObject(process, 0) } {
                WAIT_OBJECT_0 => Ok(false),
                WAIT_TIMEOUT => Ok(true),
                _ => Err(std::io::Error::last_os_error()),
            }
        })();
        // SAFETY: close this function's single owned handle exactly once.
        unsafe { CloseHandle(process) };
        result
    }

    /// The windows a person would call "open apps": visible, not owned by
    /// another window, not tool windows, not hidden by the system, with a
    /// title, and not part of the shell or of Custodian itself. Frontmost first.
    pub fn open_windows() -> Vec<AppWindow> {
        let own_pid = std::process::id();
        top_level_windows()
            .into_iter()
            .filter_map(|hwnd| {
                // SAFETY: plain queries on a handle EnumWindows just returned.
                let visible = unsafe { IsWindowVisible(hwnd) } != 0;
                if !visible {
                    return None;
                }
                // SAFETY: as above.
                let owner = unsafe { GetWindow(hwnd, GW_OWNER) };
                // SAFETY: as above.
                let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
                if !owner.is_null() || ex_style & WS_EX_TOOLWINDOW != 0 || cloaked(hwnd) {
                    return None;
                }
                if SHELL_CLASSES.contains(&class_name(hwnd).as_str()) {
                    return None;
                }
                let title = title(hwnd);
                if title.trim().is_empty() {
                    return None;
                }
                let mut pid = 0u32;
                // SAFETY: `pid` is a writable u32.
                unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
                if pid == 0 || pid == own_pid {
                    return None;
                }
                Some(AppWindow {
                    handle: hwnd as isize,
                    pid,
                    process_generation: process_generation(pid),
                    title,
                    exe: exe_of(pid),
                })
            })
            .collect()
    }

    /// Asks the window to close, as its X button does. Returns whether the
    /// request was delivered (it is not, for example, to apps running as
    /// administrator).
    pub fn request_close(handle: isize) -> bool {
        let hwnd = handle as HWND;
        // SAFETY: IsWindow safely checks a possibly stale handle, and
        // PostMessageW takes no pointers.
        unsafe { IsWindow(hwnd) != 0 && PostMessageW(hwnd, WM_CLOSE, 0, 0) != 0 }
    }

    /// Whether the window still exists.
    pub fn is_open(handle: isize) -> bool {
        // SAFETY: IsWindow safely checks a possibly stale handle.
        unsafe { IsWindow(handle as HWND) != 0 }
    }
}

#[cfg(not(windows))]
pub fn open_windows() -> Vec<AppWindow> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn request_close(_handle: isize) -> bool {
    false
}

#[cfg(not(windows))]
pub fn is_open(_handle: isize) -> bool {
    false
}

#[cfg(not(windows))]
pub fn process_generation(_pid: u32) -> Option<u64> {
    None
}

#[cfg(not(windows))]
pub fn process_identity(_pid: u32) -> Option<ProcessIdentity> {
    None
}

#[cfg(not(windows))]
pub fn probe_owned_window() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows required",
    ))
}

#[cfg(not(windows))]
pub fn process_is_running(_: u32, _: u64) -> std::io::Result<bool> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows required",
    ))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn process_probe_measures_only_own_process_without_window_content() {
        let pid = std::process::id();
        let identity = process_identity(pid).unwrap();
        assert_eq!(identity.pid, pid);
        assert_eq!(Some(identity.creation_time), process_generation(pid));
        assert!(std::path::Path::new(&identity.executable).is_absolute());
        assert_eq!(identity, process_identity(pid).unwrap());
        assert!(process_is_running(pid, identity.creation_time).unwrap());
        assert!(!process_is_running(pid, identity.creation_time.wrapping_add(1)).unwrap());
        assert!(process_identity(0).is_none());
    }

    #[test]
    fn lists_real_windows_without_the_shell_or_itself() {
        for window in open_windows() {
            assert!(!window.title.trim().is_empty());
            assert_ne!(window.pid, std::process::id());
            assert!(is_open(window.handle));
        }
    }

    #[test]
    fn a_closed_handle_is_neither_open_nor_closable() {
        // Handle values are multiples of 4 and this one is never valid.
        assert!(!is_open(0x7fff_fff0));
        assert!(!request_close(0x7fff_fff0));
        assert!(!request_close(0));
    }
}
