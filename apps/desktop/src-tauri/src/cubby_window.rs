use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

pub const CUBBY_LABEL: &str = "cubby";

/// Creates the frameless, always-on-top Cubby companion window.
pub fn create_cubby_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, CUBBY_LABEL, WebviewUrl::App("index.html".into()))
        .title("Custodian")
        .inner_size(800.0, 600.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build()?;
    Ok(())
}
