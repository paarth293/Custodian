use tauri::{Window, PhysicalPosition};

pub fn update_placement(window: &Window, calm_mode: bool) {
    if calm_mode {
        let _ = window.set_position(PhysicalPosition::new(100, 100));
    } else {
        let _ = window.set_position(PhysicalPosition::new(50, 50));
    }
}
