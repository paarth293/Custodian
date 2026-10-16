/// Expands a tool template into an argv vector.
///
/// Replaces `{param}` placeholders with the supplied arguments.
pub fn build_argv(template: &str, args: &[(String, String)]) -> Vec<String> {
    let mut parts: Vec<String> = template.split_whitespace().map(String::from).collect();
    for part in parts.iter_mut() {
        for (key, val) in args {
            let placeholder = format!("{{{key}}}");
            if part.contains(&placeholder) {
                *part = part.replace(&placeholder, val);
            }
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_arguments() {
        let args = vec![
            ("in".to_string(), "input.txt".to_string()),
            ("out".to_string(), "output.bin".to_string()),
        ];
        let argv = build_argv("tool.exe -i {in} -o {out}", &args);
        assert_eq!(
            argv,
            vec!["tool.exe", "-i", "input.txt", "-o", "output.bin"]
        );
    }
}
