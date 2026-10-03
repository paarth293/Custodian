use crate::hello;

/// Signs an approval for `plan_hash` after Windows Hello verification.
///
/// Placeholder: returns a tagged string until the engine signing key lands.
#[tauri::command]
pub fn sign_approval(plan_hash: String, method: String) -> Result<String, String> {
    if !hello::verify_hello() {
        return Err("Windows Hello verification failed".into());
    }
    Ok(format!("signed_{plan_hash}_{method}"))
}
