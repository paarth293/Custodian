use crate::cubby_window::CUBBY_LABEL;
use tauri::{AppHandle, Manager, PhysicalPosition};

/// Moves Cubby out of the way in calm mode.
#[tauri::command]
pub fn set_calm_mode(app: AppHandle, calm: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(CUBBY_LABEL)
        .ok_or("Cubby window is not open")?;
    let position = if calm {
        PhysicalPosition::new(100, 100)
    } else {
        PhysicalPosition::new(50, 50)
    };
    window.set_position(position).map_err(|err| err.to_string())
}
