/// Reverses a recorded install recipe.
///
/// F31: Returns an explicit error rather than claiming fake success.
pub fn undo_install(recipe: &str) -> Result<(), String> {
    let _ = recipe;
    Err("install undo is not implemented in this build".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unsupported_install_undo() {
        assert!(undo_install("sample_recipe").is_err());
    }
}
