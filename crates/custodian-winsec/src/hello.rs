//! Windows Hello user verification (D-14).
//!
//! The Hello prompt is drawn by a separate system process, so neither script
//! in a webview nor another program running as the user can answer it: it
//! takes the person's face, fingerprint or PIN.

/// The result of asking for Windows Hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloOutcome {
    /// The person verified.
    Verified,
    /// The person cancelled, or verification failed.
    Declined,
    /// Hello is not set up on this PC, or could not be started.
    Unavailable,
}

/// True when Windows Hello is set up and ready on this PC.
#[cfg(windows)]
pub fn available() -> bool {
    use windows::Security::Credentials::UI::{
        UserConsentVerifier, UserConsentVerifierAvailability,
    };
    UserConsentVerifier::CheckAvailabilityAsync()
        .and_then(|op| op.join())
        .is_ok_and(|a| a == UserConsentVerifierAvailability::Available)
}

/// Asks the person to verify with Windows Hello, showing `message`. The
/// prompt is owned by `owner` (a raw window handle), which brings it to the
/// front. Blocks until the person answers.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn verify(owner: isize, message: &str) -> HelloOutcome {
    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows_future::IAsyncOperation;

    if !available() {
        return HelloOutcome::Unavailable;
    }
    let Ok(interop) = factory::<UserConsentVerifier, IUserConsentVerifierInterop>() else {
        return HelloOutcome::Unavailable;
    };
    // SAFETY: `owner` is a window handle supplied by the caller; Windows
    // validates it and fails the call (handled below) if it is not a window.
    // The message is a valid HSTRING for the duration of the call.
    let request: windows::core::Result<IAsyncOperation<UserConsentVerificationResult>> = unsafe {
        interop.RequestVerificationForWindowAsync(HWND(owner as *mut _), &HSTRING::from(message))
    };
    match request.and_then(|op| op.join()) {
        Ok(UserConsentVerificationResult::Verified) => HelloOutcome::Verified,
        Ok(_) => HelloOutcome::Declined,
        Err(_) => HelloOutcome::Unavailable,
    }
}

#[cfg(not(windows))]
pub fn available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn verify(_owner: isize, _message: &str) -> HelloOutcome {
    HelloOutcome::Unavailable
}

// No unit tests: every call to `verify` shows a real prompt to the person at
// the PC. `examples/hello.rs` is the manual check.
