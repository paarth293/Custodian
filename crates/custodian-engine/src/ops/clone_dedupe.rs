use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct CloneDedupeOp;

impl CloneDedupeOp {
    pub fn execute(source: &SafePath, target: &SafePath) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        {
            // FSCTL_DUPLICATE_EXTENTS_TO_FILE block cloning mock
        }
        
        fs::copy(source.as_path(), target.as_path())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_clone_dedupe_fallback() {
        let temp_dir = std::env::temp_dir();
        let src = temp_dir.join("clone_src.txt");
        let dest = temp_dir.join("clone_dest.txt");
        
        let mut file = File::create(&src).unwrap();
        file.write_all(b"block data").unwrap();

        let safe_src = SafePath::new(&src, &temp_dir).unwrap();
        let safe_dest = SafePath::new(&dest, &temp_dir).unwrap();
        
        CloneDedupeOp::execute(&safe_src, &safe_dest).unwrap();

        let data = fs::read(&dest).unwrap();
        assert_eq!(data, b"block data");
    }
}
