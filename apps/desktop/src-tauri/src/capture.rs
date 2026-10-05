//! Keeps Cubby out of screen sharing, recordings and screenshots (SR-F7), so
//! file names in the chat never show up on someone else's screen. For a demo
//! the person can allow capture from the tray; it switches itself off again
//! after an hour (WIN-18), so it is never left on by accident.

use crate::cubby_window::CUBBY_LABEL;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// How long screenshots stay allowed before Cubby hides from them again.
pub const ALLOW_FOR: Duration = Duration::from_secs(60 * 60);

/// Bumped on every change, so an old timer never undoes a newer choice.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Whether screenshots of Cubby are allowed right now.
static ALLOWED: AtomicBool = AtomicBool::new(false);

/// Whether screen capture of Cubby is allowed right now (normally not).
pub fn is_allowed() -> bool {
    ALLOWED.load(Ordering::SeqCst)
}

/// Whether a timer started at `started` may still switch capture off.
fn timer_is_current(started: u64, now: u64) -> bool {
    started == now
}

/// Allows or blocks screen capture of Cubby's window and keeps the tray's
/// check mark in step.
pub fn set_capture_allowed(app: &AppHandle, allowed: bool) {
    if let Some(window) = app.get_webview_window(CUBBY_LABEL) {
        let _ = window.set_content_protected(!allowed);
    }
    ALLOWED.store(allowed, Ordering::SeqCst);
    crate::tray::sync_capture_check(app, allowed);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if allowed {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(ALLOW_FOR);
            if timer_is_current(generation, GENERATION.load(Ordering::SeqCst)) {
                set_capture_allowed(&app, false);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_timer_may_switch_capture_off() {
        assert!(timer_is_current(3, 3));
        // The person changed their mind after this timer started.
        assert!(!timer_is_current(3, 4));
    }
}
