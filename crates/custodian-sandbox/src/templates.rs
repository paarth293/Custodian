/// Expands a tool template into an argv vector.
///
/// Placeholder: returns a fixed argv until the template language lands.
pub fn build_argv(template: &str, args: &[(String, String)]) -> Vec<String> {
    let _ = (template, args);
    vec!["tool.exe".into(), "-i".into(), "in/file".into()]
}
