//! Custodian desktop shell: the Cubby window, tray menu and UI commands.

mod approval_service;
mod approval_window;
mod clipboard;
mod cubby_window;
mod hello;
mod input_hook;
mod placement;
mod tray;

#[tauri::command]
fn invoke_broker(action: String) -> String {
    format!("Broker received: {action}")
}

/// Placeholder ledger entries until the engine's read API is wired in.
#[tauri::command]
fn get_ledger() -> Vec<String> {
    vec![
        r"Tx 1: Moved C:\test to C:\vault".to_string(),
        r"Tx 2: Deleted D:\evil.exe".to_string(),
    ]
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            cubby_window::create_cubby_window(app.handle())?;
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_broker,
            get_ledger,
            approval_service::sign_approval,
            approval_window::request_approval,
            clipboard::set_private_text,
            placement::set_calm_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Custodian desktop shell");
}
