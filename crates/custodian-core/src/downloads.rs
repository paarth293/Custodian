/// Verifies a downloaded file before it is handed to the user.
///
/// Placeholder: flags the EICAR test file by name until the Defender
/// (`IAmsiStream` / `MpCmdRun`) integration lands.
pub fn verify_download(path: &str) -> Result<(), String> {
    if path.to_ascii_lowercase().contains("eicar") {
        return Err("Defender flagged".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_eicar() {
        assert!(verify_download(r"C:\Downloads\EICAR.com").is_err());
        assert!(verify_download(r"C:\Downloads\setup.exe").is_ok());
    }
}
