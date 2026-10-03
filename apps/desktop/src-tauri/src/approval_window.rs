use tauri::{Manager, WindowBuilder, WindowUrl};

pub fn open_approval_window(app: &tauri::AppHandle, plan_hash: String) {
    let _window = WindowBuilder::new(
        app,
        "approval_window",
        WindowUrl::App(format!("approval.html?hash={}", plan_hash).into())
    )
    .title("Approval Required")
    .build().unwrap();
}
