//! Custodian desktop shell: the Cubby window, tray menu and UI commands.

mod agent;
mod approval_service;
mod approval_window;
mod assistant;
mod clipboard;
mod cubby_window;
mod hello;
mod input_hook;
mod layout;
mod mode;
mod placement;
mod tray;

#[tauri::command]
fn invoke_broker(action: String) -> String {
    format!("Broker received: {action}")
}

/// Commands exposed to webviews. Approval signing is intentionally absent:
/// see `approval_service`.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(mode::ModeState::default())
        .manage(assistant::AssistantState::default())
        .manage(cubby_window::CubbyWindowState::default())
        .setup(|app| {
            cubby_window::create_cubby_window(app.handle())?;
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_broker,
            assistant::get_ledger,
            assistant::send_chat_message,
            assistant::approve_plan,
            assistant::reject_plan,
            clipboard::set_private_text,
            placement::set_calm_mode,
            cubby_window::set_panel_open,
            cubby_window::set_click_through,
            cubby_window::hide_cubby,
            cubby_window::quit_app,
            mode::get_mode,
            mode::set_demo_mode,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Custodian desktop shell")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                cubby_window::persist(app);
            }
        });
}
