pub struct AnchorManager;

impl AnchorManager {
    pub fn save_anchor(height: u64, root_hash: &[u8; 32]) -> Result<(), &'static str> {
        println!("Saving anchor at height {} with hash {:?}", height, root_hash);
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
    fn test_anchor_manager() {
        assert!(AnchorManager::save_anchor(1, &[0u8; 32]).is_ok());
        let (height, _hash) = AnchorManager::load_anchor().unwrap();
        assert_eq!(height, 0); 
    }
}
