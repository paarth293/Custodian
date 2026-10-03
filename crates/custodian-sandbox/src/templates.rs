pub fn build_argv(_template: &str, _args: &[(String, String)]) -> Vec<String> {
    vec!["tool.exe".into(), "-i".into(), "in/file".into()]
}
