use custodian_winsec::SafePath;
use std::fs;
use std::io;

pub struct MoveOutputsOp;

impl MoveOutputsOp {
    pub fn execute(source: &SafePath, target: &SafePath) -> io::Result<()> {
        fs::rename(source.as_path(), target.as_path())
    }
    pub fn inverse(target: &SafePath) -> io::Result<()> {
        fs::remove_file(target.as_path())
    }
}
