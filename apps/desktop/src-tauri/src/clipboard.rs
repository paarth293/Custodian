use tauri::command;

#[command]
pub fn set_private_text(text: String) {
    println!("Private clipboard set: {}", text);
}
