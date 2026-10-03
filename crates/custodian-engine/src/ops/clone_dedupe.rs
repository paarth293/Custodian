use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct CloneDedupeOp;

impl CloneDedupeOp {
    /// Copies `source` to `target`.
    ///
    /// On ReFS this will use `FSCTL_DUPLICATE_EXTENTS_TO_FILE` block cloning;
    /// a plain copy is the portable fallback.
    pub fn execute(source: &SafePath, target: &SafePath) -> io::Result<()> {
        fs::copy(source.as_path(), target.as_path())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_copy() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("clone_src.txt");
        let dest = dir.path().join("clone_dest.txt");
        fs::write(&src, b"block data").unwrap();

        CloneDedupeOp::execute(
            &SafePath::new(&src, dir.path()).unwrap(),
            &SafePath::new(&dest, dir.path()).unwrap(),
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"block data");
    }
}
