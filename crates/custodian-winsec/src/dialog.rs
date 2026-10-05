//! Native confirmation prompts.
//!
//! A native dialog lives outside every webview, so script running in the UI
//! cannot click it: an approval shown here needs a person (A-09.1).

/// Asks the user a yes/no question in a native, top-most dialog. "No" is the
/// default button, so a stray Enter never approves anything.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn confirm(title: &str, text: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    };

    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (title, text) = (wide(title), wide(text));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the
    // call, and a null owner window is allowed.
    let choice = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2 | MB_TOPMOST | MB_SETFOREGROUND,
        )
    };
    choice == IDYES
}

/// Without a native dialog nothing can be confirmed.
#[cfg(not(windows))]
pub fn confirm(_title: &str, _text: &str) -> bool {
    false
}
