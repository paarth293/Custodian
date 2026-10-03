/// Persists the ledger's latest height and root hash to an external anchor.
///
/// Placeholder: anchoring is not implemented; `load_anchor` always returns
/// the genesis anchor.
pub struct AnchorManager;

impl AnchorManager {
    pub fn save_anchor(height: u64, root_hash: &[u8; 32]) -> Result<(), &'static str> {
        let _ = (height, root_hash);
        Ok(())
    }

    pub fn load_anchor() -> Result<(u64, [u8; 32]), &'static str> {
        Ok((0, [0u8; 32]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_anchor() {
        assert!(AnchorManager::save_anchor(1, &[0u8; 32]).is_ok());
        assert_eq!(AnchorManager::load_anchor().unwrap(), (0, [0u8; 32]));
    }
}
