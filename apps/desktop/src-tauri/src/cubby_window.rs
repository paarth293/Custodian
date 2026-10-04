//! The Cubby companion window: a transparent, always-on-top, non-focusable
//! window that remembers where you left it and grows a chat panel on the side
//! with room.

use crate::layout::{self, PanelSide, Rect};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const CUBBY_LABEL: &str = "cubby";

/// Logical size of the window with only Cubby (room for the speech bubble).
const COMPACT: (f64, f64) = (240.0, 280.0);
/// Logical size with the chat panel open.
const EXPANDED: (f64, f64) = (620.0, 580.0);
const SAVE_INTERVAL: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct CubbyWindowState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Compact rectangle to return to, and the expanded window's position,
    /// while the panel is open.
    expanded: Option<(Rect, PhysicalPosition<i32>, PanelSide)>,
    last_saved: Option<Instant>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedPosition {
    x: i32,
    y: i32,
}

fn state_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("cubby-window.json"))
}

fn load_position(app: &AppHandle) -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(state_file(app)?).ok()?;
    let saved: SavedPosition = serde_json::from_str(&text).ok()?;
    Some((saved.x, saved.y))
}

fn save_position(app: &AppHandle, position: PhysicalPosition<i32>) {
    let Some(path) = state_file(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let saved = SavedPosition {
        x: position.x,
        y: position.y,
    };
    if let Ok(json) = serde_json::to_string(&saved) {
        let _ = std::fs::write(path, json);
    }
}

fn to_rect(area: &tauri::PhysicalRect<i32, u32>) -> Rect {
    Rect {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width as i32,
        height: area.size.height as i32,
    }
}

fn physical(size: (f64, f64), scale: f64) -> (i32, i32) {
    (
        (size.0 * scale).round() as i32,
        (size.1 * scale).round() as i32,
    )
}

/// Creates the Cubby window at its remembered (or default) position.
pub fn create_cubby_window(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, CUBBY_LABEL, WebviewUrl::App("index.html".into()))
        .title("Cubby")
        .inner_size(COMPACT.0, COMPACT.1)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focusable(false)
        .visible(false)
        .build()?;

    if let Some(primary) = window.primary_monitor()? {
        let scale = primary.scale_factor();
        let work_areas: Vec<Rect> = window
            .available_monitors()?
            .iter()
            .map(|m| to_rect(m.work_area()))
            .collect();
        let (x, y) = layout::initial_position(
            load_position(app),
            physical(COMPACT, scale),
            &work_areas,
            to_rect(primary.work_area()),
        );
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    window.show()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Moved(position) = event {
            on_moved(&handle, *position);
        }
    });
    Ok(())
}

fn on_moved(app: &AppHandle, position: PhysicalPosition<i32>) {
    let state = app.state::<CubbyWindowState>();
    let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());

    if let Some((compact, last, _)) = inner.expanded.as_mut() {
        // Dragging with the panel open moves the compact anchor too.
        compact.x += position.x - last.x;
        compact.y += position.y - last.y;
        *last = position;
        return;
    }

    let due = inner
        .last_saved
        .is_none_or(|t| t.elapsed() >= SAVE_INTERVAL);
    if due {
        inner.last_saved = Some(Instant::now());
        drop(inner);
        save_position(app, position);
    }
}

/// Saves the final position on exit (moves are throttled while dragging).
pub fn persist(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CUBBY_LABEL) else {
        return;
    };
    let state = app.state::<CubbyWindowState>();
    let inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
    let position = match inner.expanded {
        Some((compact, _, _)) => PhysicalPosition::new(compact.x, compact.y),
        None => match window.outer_position() {
            Ok(p) => p,
            Err(_) => return,
        },
    };
    drop(inner);
    save_position(app, position);
}

fn cubby(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(CUBBY_LABEL)
        .ok_or_else(|| "Cubby window is not open".to_string())
}

/// Opens or closes the chat panel. Returns the side the panel is on.
#[tauri::command]
pub fn set_panel_open(app: AppHandle, open: bool) -> Result<PanelSide, String> {
    let window = cubby(&app)?;
    let state = app.state::<CubbyWindowState>();
    let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
    let err = |e: tauri::Error| e.to_string();

    if open {
        if let Some((_, _, side)) = inner.expanded {
            return Ok(side);
        }
        let position = window.outer_position().map_err(err)?;
        let size = window.outer_size().map_err(err)?;
        let monitor = window
            .current_monitor()
            .map_err(err)?
            .ok_or("no monitor for the Cubby window")?;
        let compact = Rect {
            x: position.x,
            y: position.y,
            width: size.width as i32,
            height: size.height as i32,
        };
        let (target, side) = layout::expand(
            compact,
            physical(EXPANDED, monitor.scale_factor()),
            to_rect(monitor.work_area()),
        );
        let target_pos = PhysicalPosition::new(target.x, target.y);
        inner.expanded = Some((compact, target_pos, side));
        drop(inner);
        window.set_position(target_pos).map_err(err)?;
        window
            .set_size(PhysicalSize::new(target.width as u32, target.height as u32))
            .map_err(err)?;
        let _ = window.set_focusable(true);
        let _ = window.set_focus();
        Ok(side)
    } else {
        let Some((compact, _, side)) = inner.expanded.take() else {
            return Ok(PanelSide::Right);
        };
        drop(inner);
        window
            .set_size(PhysicalSize::new(
                compact.width as u32,
                compact.height as u32,
            ))
            .map_err(err)?;
        window
            .set_position(PhysicalPosition::new(compact.x, compact.y))
            .map_err(err)?;
        let _ = window.set_focusable(false);
        save_position(&app, PhysicalPosition::new(compact.x, compact.y));
        Ok(side)
    }
}

/// Makes the transparent parts of the window click-through (or not).
#[tauri::command]
pub fn set_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    cubby(&app)?
        .set_ignore_cursor_events(enabled)
        .map_err(|e| e.to_string())
}

/// Hides Cubby; the tray's "Show Cubby" brings it back.
#[tauri::command]
pub fn hide_cubby(app: AppHandle) -> Result<(), String> {
    cubby(&app)?.hide().map_err(|e| e.to_string())
}

/// Quits Custodian, saving Cubby's position first.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    persist(&app);
    app.exit(0);
}
