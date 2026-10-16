/// Status of Authenticode publisher verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublisherStatus {
    Verified(String),
    Mismatch { expected: String, actual: String },
    Unavailable(String),
}

/// Checks an installer's Authenticode publisher.
///
/// F29: Replaced placeholder constant `true`. Until `WinVerifyTrust` integration lands,
/// returns `PublisherStatus::Unavailable` rather than claiming the publisher was verified.
pub fn verify_publisher(path: &str, expected_publisher: &str) -> PublisherStatus {
    let _ = (path, expected_publisher);
    PublisherStatus::Unavailable("WinVerifyTrust publisher verification unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publisher_verification_reports_unavailable_rather_than_false_safe() {
        assert_eq!(
            verify_publisher("setup.exe", "Trusted Corp"),
            PublisherStatus::Unavailable(
                "WinVerifyTrust publisher verification unavailable".into()
            )
        );
    }
}
