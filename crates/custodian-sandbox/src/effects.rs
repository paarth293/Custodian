pub fn verify_effects(declared: &[String], actual: &[String]) -> bool {
    actual.iter().all(|a| declared.contains(a))
}
