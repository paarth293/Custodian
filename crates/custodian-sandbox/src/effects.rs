/// Returns `true` when every observed effect was declared up front.
pub fn verify_effects(declared: &[String], actual: &[String]) -> bool {
    actual.iter().all(|a| declared.contains(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_undeclared_effects() {
        let declared = vec!["write:out".to_string()];
        assert!(verify_effects(&declared, &["write:out".to_string()]));
        assert!(!verify_effects(&declared, &["net:connect".to_string()]));
    }
}
