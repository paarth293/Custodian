pub fn is_safe_mode(crash_count: u32) -> bool {
    crash_count >= 3
}
