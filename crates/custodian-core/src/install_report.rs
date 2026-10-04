/// Verdict for an install-lab run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallVerdict {
    Clean,
    Blocked,
}

/// Blocks any install whose diff adds a trusted root certificate.
pub fn generate_report(diff: &str) -> InstallVerdict {
    if diff.contains("root cert") {
        InstallVerdict::Blocked
    } else {
        InstallVerdict::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_root_cert_install() {
        assert_eq!(
            generate_report("added root cert: Evil CA"),
            InstallVerdict::Blocked
        );
        assert_eq!(
            generate_report("added file: app.exe"),
            InstallVerdict::Clean
        );
    }
}
