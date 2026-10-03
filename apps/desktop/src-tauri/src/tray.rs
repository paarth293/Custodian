use crate::cubby_window::CUBBY_LABEL;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

/// Adds the tray icon with Show / Hide / Quit actions for Cubby.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Cubby", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide Cubby", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" | "hide" => {
                if let Some(window) = app.get_webview_window(CUBBY_LABEL) {
                    let _ = if event.id().as_ref() == "show" {
                        window.show()
                    } else {
                        window.hide()
                    };
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
