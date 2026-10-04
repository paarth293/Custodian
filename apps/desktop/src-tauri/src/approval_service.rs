//! Approval signing. Deliberately **not** a webview command: the approval key
//! must never be reachable from a webview (build plan step 84, T-KEYS-02).
//! The engine dispatcher calls this after the approval window reports a
//! genuine hardware click.

use crate::{hello, input_hook};

/// Signs an approval for `plan_hash` after a real click and Windows Hello.
///
/// Placeholder: returns a tagged string until the DPAPI-held key lands.
#[allow(dead_code)] // Called by the engine dispatcher, which is not wired in yet.
pub fn sign_approval(plan_hash: &str, method: &str) -> Result<String, String> {
    if !input_hook::verify_real_click() {
        return Err("approval requires a genuine click".into());
    }
    if !hello::verify_hello() {
        return Err("Windows Hello verification failed".into());
    }
    Ok(format!("signed_{plan_hash}_{method}"))
}
