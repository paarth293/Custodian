/// Prepares Custodian for uninstall, optionally keeping user data.
///
/// F31: Returns an explicit error rather than claiming fake success.
pub fn prepare_uninstall(keep_files: bool) -> Result<(), String> {
    let _ = keep_files;
    Err("uninstall preparation is not implemented in this build".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unsupported_uninstall_prep() {
        assert!(prepare_uninstall(true).is_err());
    }
}
