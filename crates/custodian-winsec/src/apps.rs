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
    pub title: String,
    /// The program's file name, such as "notepad.exe" (empty if unknown).
    pub exe: String,
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
pub use imp::{is_open, open_windows, request_close};

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::{AppWindow, SHELL_CLASSES};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

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
