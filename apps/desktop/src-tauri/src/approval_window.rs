use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const APPROVAL_LABEL: &str = "approval";

/// Shows the approval dialog for a plan, reusing the window if it is already
/// open. Opened from Rust (the engine dispatcher), never from a webview.
#[allow(dead_code)] // Called by the engine dispatcher, which is not wired in yet.
pub fn open_approval_window(app: &AppHandle, plan_hash: &str) -> Result<(), String> {
    if plan_hash.is_empty() || !plan_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("plan hash must be hexadecimal".into());
    }
    let url = format!("index.html?approve={plan_hash}");

    if let Some(window) = app.get_webview_window(APPROVAL_LABEL) {
        let current = window.url().map_err(|err| err.to_string())?;
        let target = current.join(&url).map_err(|err| err.to_string())?;
        window.navigate(target).map_err(|err| err.to_string())?;
        return window.set_focus().map_err(|err| err.to_string());
    }

    WebviewWindowBuilder::new(app, APPROVAL_LABEL, WebviewUrl::App(url.into()))
        .title("Approval Required")
        .always_on_top(true)
        .build()
        .map(|_| ())
        .map_err(|err| err.to_string())
}
