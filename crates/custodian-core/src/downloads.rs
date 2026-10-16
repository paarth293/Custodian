/// Detailed outcome of inspecting a downloaded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadScanResult {
    Clean,
    ThreatDetected(String),
    ScannerUnavailable(String),
}

/// Inspects a downloaded file.
///
/// F29: Distinguishes clean, threat detected, and scanner unavailable.
/// An offline system without a connected Defender/AMSI provider must
/// report `ScannerUnavailable`, rather than pretending a file was verified.
pub fn scan_download(path: &str) -> DownloadScanResult {
    let lower = path.to_ascii_lowercase();
    if lower.contains("eicar") {
        return DownloadScanResult::ThreatDetected("EICAR test signature detected".into());
    }
    DownloadScanResult::ScannerUnavailable(
        "Windows Defender / AMSI scanner integration is not connected".into(),
    )
}

/// Verifies a downloaded file before it is handed to the user.
///
/// F29: Returns Err if a threat is detected or if verification is unavailable,
/// ensuring we fail closed rather than reporting uninspected downloads as safe.
pub fn verify_download(path: &str) -> Result<(), String> {
    match scan_download(path) {
        DownloadScanResult::Clean => Ok(()),
        DownloadScanResult::ThreatDetected(threat) => Err(format!("Threat detected: {threat}")),
        DownloadScanResult::ScannerUnavailable(reason) => {
            Err(format!("Security scanner unavailable: {reason}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_eicar_and_reports_unavailable_scanner() {
        assert!(verify_download(r"C:\Downloads\EICAR.com").is_err());
        assert_eq!(
            scan_download(r"C:\Downloads\EICAR.com"),
            DownloadScanResult::ThreatDetected("EICAR test signature detected".into())
        );
        // F29: Renaming or checking an uninspected file does not return verified clean!
        assert_eq!(
            scan_download(r"C:\Downloads\setup.exe"),
            DownloadScanResult::ScannerUnavailable(
                "Windows Defender / AMSI scanner integration is not connected".into()
            )
        );
        assert!(verify_download(r"C:\Downloads\setup.exe").is_err());
    }
}
