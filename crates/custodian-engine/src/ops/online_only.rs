use custodian_winsec::SafePath;
use std::io;

pub struct OnlineOnlyOp;

impl OnlineOnlyOp {
    /// Marks a cloud-synced file as online-only.
    ///
    /// Returns Unsupported until `CfSetPinState` integration lands.
    pub fn execute(target: &SafePath) -> io::Result<()> {
        let _ = target;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "online_only is not implemented: CfSetPinState is unavailable",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_online_only_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("cloud.txt");
        std::fs::write(&file_path, b"data").unwrap();
        let safe = SafePath::new(&file_path, dir.path()).unwrap();
        let res = OnlineOnlyOp::execute(&safe);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), io::ErrorKind::Unsupported);
    }
}
