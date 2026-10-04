//! Custodian desktop shell: the Cubby window, tray menu and UI commands.

mod approval_service;
mod approval_window;
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

/// Placeholder ledger entries until the engine's read API is wired in.
#[tauri::command]
fn get_ledger() -> Vec<String> {
    vec![
        r"Tx 1: Moved C:\test to C:\vault".to_string(),
        r"Tx 2: Deleted D:\evil.exe".to_string(),
    ]
}

#[tauri::command]
async fn send_chat_message(
    message: String,
    channel: tauri::ipc::Channel<String>,
) -> Result<(), String> {
    // Phase 2.3: Placeholder for Node Agent IPC stream.
    // This perfectly mimics the stream layout until the agent process manager is wired.
    let reply = format!(
        "Hello! I received your message: \"{}\". I am currently running in offline mode.",
        message
    );
    for word in reply.split_whitespace() {
        channel
            .send(format!("{} ", word))
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

#[tauri::command]
fn approve_plan(plan_id: String) -> Result<(), String> {
    // Phase 4: Validates the physical click and issues the signature to the Engine.
    println!("Plan {} approved by user.", plan_id);
    Ok(())
}

/// Commands exposed to webviews. Approval signing is intentionally absent:
/// see `approval_service`.
pub fn run() {
    tauri::Builder::default()
        .manage(mode::ModeState::default())
        .manage(cubby_window::CubbyWindowState::default())
        .setup(|app| {
            cubby_window::create_cubby_window(app.handle())?;
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_broker,
            get_ledger,
            send_chat_message,
            approve_plan,
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
