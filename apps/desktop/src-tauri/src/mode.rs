//! Offline/online mode as shown by Cubby's colour (D-17).
//!
//! `online` must reflect the real broker state; until the broker process is
//! wired in it is always false. `demo` lets the user preview the amber
//! Special Form from the tray or Cubby's menu, and the UI labels it as a demo.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

pub const MODE_EVENT: &str = "mode-changed";

#[derive(Default)]
pub struct ModeState {
    online: AtomicBool,
    demo: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Mode {
    pub online: bool,
    pub demo: bool,
}

impl ModeState {
    pub fn snapshot(&self) -> Mode {
        Mode {
            online: self.online.load(Ordering::Relaxed),
            demo: self.demo.load(Ordering::Relaxed),
        }
    }

    fn set_demo(&self, enabled: bool) -> Mode {
        self.demo.store(enabled, Ordering::Relaxed);
        self.snapshot()
    }
}

/// Turns the Special Form preview on or off and notifies every window.
pub fn apply_demo(app: &AppHandle, enabled: bool) -> Mode {
    let mode = app.state::<ModeState>().set_demo(enabled);
    let _ = app.emit(MODE_EVENT, mode);
    mode
}

#[tauri::command]
pub fn get_mode(state: tauri::State<'_, ModeState>) -> Mode {
    state.snapshot()
}

#[tauri::command]
pub fn set_demo_mode(app: AppHandle, enabled: bool) -> Mode {
    crate::tray::sync_demo_check(&app, enabled);
    apply_demo(&app, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_offline_without_demo() {
        let state = ModeState::default();
        assert_eq!(
            state.snapshot(),
            Mode {
                online: false,
                demo: false
            }
        );
    }

    #[test]
    fn demo_does_not_claim_to_be_online() {
        let state = ModeState::default();
        let mode = state.set_demo(true);
        assert!(mode.demo);
        assert!(!mode.online);
    }
}
