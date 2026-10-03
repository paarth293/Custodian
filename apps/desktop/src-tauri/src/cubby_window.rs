use tauri::{Manager, WindowBuilder, WindowUrl};

pub fn create_cubby_window(app: &mut tauri::App) -> tauri::Result<()> {
    let _window = WindowBuilder::new(
        app,
        "cubby",
        WindowUrl::App("index.html".into())
    )
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .build()?;
    
    Ok(())
}
