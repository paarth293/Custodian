use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

pub const CUBBY_LABEL: &str = "cubby";

/// Creates the frameless, always-on-top Cubby companion window.
pub fn create_cubby_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, CUBBY_LABEL, WebviewUrl::App("index.html".into()))
        .title("Custodian Companion")
        .inner_size(120.0, 120.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focusable(false) // Verified: Tauri 2 provides focusable(false) which prevents focus stealing
        .build()?;
    Ok(())
}
