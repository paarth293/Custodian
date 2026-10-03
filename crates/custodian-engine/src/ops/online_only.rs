use custodian_winsec::SafePath;
use std::io;

pub struct OnlineOnlyOp;

impl OnlineOnlyOp {
    /// Marks a cloud-synced file as online-only.
    ///
    /// Placeholder for `CfSetPinState`; currently a no-op.
    pub fn execute(target: &SafePath) -> io::Result<()> {
        let _ = target;
        Ok(())
    }
}
