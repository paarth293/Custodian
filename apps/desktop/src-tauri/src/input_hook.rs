//! Tracks genuine (hardware) clicks so approvals cannot be scripted.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A click is "fresh" for this long after it happens.
const CLICK_WINDOW_MS: u64 = 500;

static LAST_REAL_CLICK_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Called from the low-level mouse hook for non-injected clicks.
#[allow(dead_code)] // Wired up together with the Win32 mouse hook.
pub fn record_real_click() {
    LAST_REAL_CLICK_MS.store(now_ms(), Ordering::Relaxed);
}

/// True when a hardware click happened within the last [`CLICK_WINDOW_MS`].
#[allow(dead_code)] // Wired up together with the Win32 mouse hook.
pub fn verify_real_click() -> bool {
    let last = LAST_REAL_CLICK_MS.load(Ordering::Relaxed);
    last != 0 && now_ms().saturating_sub(last) < CLICK_WINDOW_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_is_fresh_right_after_recording() {
        record_real_click();
        assert!(verify_real_click());
    }
}
