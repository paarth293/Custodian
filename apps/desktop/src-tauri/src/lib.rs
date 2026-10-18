//! Custodian desktop shell: the Cubby window, tray menu and UI commands.

mod agent;
mod approval_service;
mod approval_window;
mod assistant;
mod capture;
mod clipboard;
mod cubby_window;
mod history_ledger;
mod input_hook;
mod layout;
mod mode;
mod paths;
mod placement;
mod tray;
use tauri::Manager;

#[tauri::command]
fn invoke_broker(action: String) -> Result<String, String> {
    let _ = action;
    Err(custodian_core::privacy::OFFLINE.into())
}

/// Commands exposed to webviews. Approval signing is intentionally absent:
/// `approve_plan` asks with Windows Hello (or a native dialog) and signs in Rust (see
/// `approval_service`).
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(mode::ModeState::default())
        .manage(cubby_window::CubbyWindowState::default())
        .setup(|app| {
            let data_dir = paths::data_dir(app.handle())?;
            // Acquire before loading any mutable policy, task or receipt state.
            // Held by Tauri until exit; a crash releases the OS lock as well.
            // This root deliberately ignores verification-profile overrides:
            // every desktop for this OS user shares authority over local files.
            let authority_dir = app.path().app_local_data_dir()?.join("authority");
            let host = match custodian_core::instance::InstanceGuard::acquire_for_profile(
                &authority_dir,
                &data_dir,
            ) {
                Ok(host) => host,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    eprintln!("[desktop] {error}");
                    app.handle().exit(0);
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };
            app.manage(host);
            app.manage(assistant::AssistantState::load(&data_dir).map_err(std::io::Error::other)?);
            cubby_window::create_cubby_window(app.handle())?;
            assistant::start_idle_unloader(app.handle().clone());
            assistant::start_safe_place_keeper(app.handle().clone());
            assistant::start_reminders(app.handle().clone());
            assistant::start_nudges(app.handle().clone());
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_broker,
            assistant::get_ledger,
            assistant::privacy::get_privacy_settings,
            assistant::privacy::set_private_paths,
            assistant::privacy::get_capabilities,
            assistant::routine::get_routine_settings,
            assistant::routine::set_routine_settings,
            assistant::send_chat_message,
            assistant::reset_conversation,
            assistant::approve_plan,
            assistant::reject_plan,
            assistant::decide_action,
            assistant::cancel_chat,
            assistant::list_active_tasks,
            assistant::cancel_task,
            assistant::pause_task,
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
