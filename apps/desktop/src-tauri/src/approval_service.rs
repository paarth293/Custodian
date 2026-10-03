use tauri::command;

#[command]
pub fn sign_approval(plan_hash: String, method: String) -> Result<String, String> {
    Ok(format!("signed_{}_{}", plan_hash, method))
}
