pub mod clipboard;
pub mod tray;
pub mod approval_service;
pub mod approval_window;
pub mod input_hook;
pub mod hello;
pub mod placement;
pub mod cubby_window;
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn invoke_broker(action: String) -> String {
    format!("Broker received: {}", action)
}

#[tauri::command]
fn get_ledger() -> Vec<String> {
    vec![
        "Tx 1: Moved C:\\test to C:\\vault".to_string(),
        "Tx 2: Deleted D:\\evil.exe".to_string(),
    ]
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![invoke_broker, get_ledger])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}




