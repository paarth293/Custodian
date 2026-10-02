use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FixtureRoot(PathBuf);

impl FixtureRoot {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let marker = path.join(".custodian-fixture-root");
        if !marker.exists() {
            panic!("SECURITY: Test attempted to use a non-fixture path. The path must contain a .custodian-fixture-root marker. Path: {:?}", path);
        }
        FixtureRoot(path.to_path_buf())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}
