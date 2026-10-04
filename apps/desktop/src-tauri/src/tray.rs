use crate::cubby_window::CUBBY_LABEL;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

/// Event the frontend listens to for "open the chat panel".
pub const OPEN_PANEL_EVENT: &str = "cubby-open-panel";

/// Keeps the tray's demo check item in sync with the Cubby menu.
pub struct TrayItems {
    demo: CheckMenuItem<Wry>,
}

pub fn sync_demo_check(app: &AppHandle, enabled: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.demo.set_checked(enabled);
    }
}

/// Adds the tray icon: talk, show/hide, Special Form preview, quit.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let talk = MenuItem::with_id(app, "talk", "Talk to Cubby", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Cubby", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide Cubby", true, None::<&str>)?;
    let demo = CheckMenuItem::with_id(
        app,
        "demo",
        "Preview Special Form (demo)",
        true,
        false,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Custodian", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&talk, &show, &hide, &demo, &separator, &quit])?;
    app.manage(TrayItems { demo: demo.clone() });

    let mut builder = TrayIconBuilder::new()
        .tooltip("Custodian")
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let window = app.get_webview_window(CUBBY_LABEL);
            match event.id().as_ref() {
                "talk" => {
                    if let Some(window) = &window {
                        let _ = window.show();
                    }
                    let _ = app.emit(OPEN_PANEL_EVENT, ());
                }
                "show" => {
                    if let Some(window) = &window {
                        let _ = window.show();
                    }
                }
                "hide" => {
                    if let Some(window) = &window {
                        let _ = window.hide();
                    }
                }
                "demo" => {
                    let enabled = demo.is_checked().unwrap_or(false);
                    crate::mode::apply_demo(app, enabled);
                }
                "quit" => {
                    crate::cubby_window::persist(app);
                    app.exit(0);
                }
                _ => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
