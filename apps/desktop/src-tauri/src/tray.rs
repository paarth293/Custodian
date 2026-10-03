use tauri::{SystemTray, SystemTrayMenu, CustomMenuItem, SystemTrayEvent, Manager};

pub fn create_tray() -> SystemTray {
    let hide = CustomMenuItem::new("hide".to_string(), "Hide Cubby");
    let show = CustomMenuItem::new("show".to_string(), "Show Cubby");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_item(hide)
        .add_item(quit);
    
    SystemTray::new().with_menu(tray_menu)
}

pub fn handle_tray_event(app: &tauri::AppHandle, event: SystemTrayEvent) {
    if let SystemTrayEvent::MenuItemClick { id, .. } = event {
        let window = app.get_window("cubby").unwrap();
        match id.as_str() {
            "hide" => window.hide().unwrap(),
            "show" => window.show().unwrap(),
            "quit" => std::process::exit(0),
            _ => {}
        }
    }
}
