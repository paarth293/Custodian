/// Places text on a private clipboard that is cleared after use.
///
/// Placeholder: intentionally does not log or store the text.
#[tauri::command]
pub fn set_private_text(text: String) {
    drop(text);
}
