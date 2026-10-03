/// Checks an installer's Authenticode publisher.
///
/// Placeholder: always `true` until `WinVerifyTrust` integration lands.
pub fn verify_publisher(path: &str, expected_publisher: &str) -> bool {
    let _ = (path, expected_publisher);
    true
}
