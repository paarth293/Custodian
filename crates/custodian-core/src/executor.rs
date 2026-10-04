use std::fs;
use std::path::Path;
use custodian_winsec::{protect_data, SafePath};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quarantine,
    Delete,
}

pub fn execute_action(
    target: impl AsRef<Path>,
    root: impl AsRef<Path>,
    action: Action,
    vault_dir: impl AsRef<Path>,
) -> Result<(), String> {
    let safe_target = SafePath::new(target.as_ref(), root)
        .map_err(|e| format!("Security violation: {}", e))?;
    
    let path = safe_target.as_path();
    if !path.exists() {
        return Err("Target file does not exist".to_string());
    }

    match action {
        Action::Delete => {
            // Basic erase. In Phase 4.2 this could be expanded to a secure multi-pass overwrite.
            fs::remove_file(path).map_err(|e| format!("Failed to delete: {}", e))?;
        }
        Action::Quarantine => {
            // 1. Read file contents
            let data = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
            
            // 2. Encrypt using Windows DPAPI (via custodian-winsec)
            let encrypted = protect_data(&data)
                .map_err(|e| format!("DPAPI encryption failed: {}", e))?;
            
            // 3. Write to vault
            let file_name = path.file_name()
                .ok_or_else(|| "Invalid file name".to_string())?;
            let vault_path = vault_dir.as_ref().join(file_name);
            
            fs::write(&vault_path, encrypted)
                .map_err(|e| format!("Failed to write to vault: {}", e))?;
                
            // 4. Remove original
            fs::remove_file(path).map_err(|e| format!("Failed to remove original: {}", e))?;
        }
    }

    Ok(())
}
