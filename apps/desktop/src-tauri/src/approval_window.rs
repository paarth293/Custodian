use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

const APPROVAL_LABEL: &str = "approval";

/// Opens the approval dialog for a plan. `plan_hash` must be hex so it can be
/// embedded in the URL without escaping.
#[tauri::command]
pub fn request_approval(app: AppHandle, plan_hash: String) -> Result<(), String> {
    if plan_hash.is_empty() || !plan_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("plan hash must be hexadecimal".into());
    }

    WebviewWindowBuilder::new(
        &app,
        APPROVAL_LABEL,
        WebviewUrl::App(format!("index.html?approve={plan_hash}").into()),
    )
    .title("Approval Required")
    .always_on_top(true)
    .build()
    .map(|_| ())
    .map_err(|err| err.to_string())
}
