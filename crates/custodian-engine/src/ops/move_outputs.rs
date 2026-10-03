use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct MoveOutputsOp;

impl MoveOutputsOp {
    pub fn execute(source: &SafePath, target: &SafePath) -> io::Result<()> {
        fs::rename(source.as_path(), target.as_path())
    }

    /// Moves `target` back to `source`.
    pub fn inverse(source: &SafePath, target: &SafePath) -> io::Result<()> {
        fs::rename(target.as_path(), source.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_restores_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("out.bin");
        let dst = dir.path().join("moved.bin");
        fs::write(&src, b"output").unwrap();
        let (src_safe, dst_safe) = (
            SafePath::new(&src, dir.path()).unwrap(),
            SafePath::new(&dst, dir.path()).unwrap(),
        );

        MoveOutputsOp::execute(&src_safe, &dst_safe).unwrap();
        assert!(!src.exists());
        MoveOutputsOp::inverse(&src_safe, &dst_safe).unwrap();
        assert_eq!(fs::read(&src).unwrap(), b"output");
    }
}
