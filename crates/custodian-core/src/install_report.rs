pub fn generate_report(diff: &str) -> String {
    if diff.contains("root cert") {
        "blocked".to_string()
    } else {
        "clean".to_string()
    }
}
