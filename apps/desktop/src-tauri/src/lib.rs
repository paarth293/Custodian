//! Custodian desktop shell: the Cubby window, tray menu and UI commands.
use tauri::Manager;

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

#[tauri::command]
fn set_ignore_cursor_events(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(cubby_window::CUBBY_LABEL) {
        window.set_ignore_cursor_events(ignore).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn send_chat_message(message: String, channel: tauri::ipc::Channel<String>) -> Result<(), String> {
    // Phase 2.3: Placeholder for Node Agent IPC stream.
    // This perfectly mimics the stream layout until the agent process manager is wired.
    let reply = format!("Hello! I received your message: \"{}\". I am currently running in offline mode.", message);
    for word in reply.split_whitespace() {
        channel.send(format!("{} ", word)).map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

/// Commands exposed to webviews. Approval signing is intentionally absent:
/// see `approval_service`.
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
            set_ignore_cursor_events,
            send_chat_message,
            clipboard::set_private_text,
            placement::set_calm_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Custodian desktop shell");
}
