/// Consecutive crashes after which Custodian starts in safe mode.
pub const SAFE_MODE_CRASH_THRESHOLD: u32 = 3;

pub fn is_safe_mode(crash_count: u32) -> bool {
    crash_count >= SAFE_MODE_CRASH_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enters_safe_mode_at_threshold() {
        assert!(!is_safe_mode(2));
        assert!(is_safe_mode(3));
    }
}
