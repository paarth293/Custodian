//! Test helpers that refuse to operate on real user folders.

use std::path::{Path, PathBuf};

/// Marker file that `custodian-fixtures` writes into every generated tree.
pub const FIXTURE_MARKER: &str = ".custodian-fixture-root";

/// A directory proven to be a generated fixture tree.
#[derive(Debug, Clone)]
pub struct FixtureRoot(PathBuf);

impl FixtureRoot {
    /// # Panics
    /// If `path` does not contain [`FIXTURE_MARKER`]. This is deliberate:
    /// a test must never touch an unmarked directory.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        assert!(
            path.join(FIXTURE_MARKER).exists(),
            "SECURITY: test attempted to use a non-fixture path (missing {FIXTURE_MARKER}): {}",
            path.display()
        );
        FixtureRoot(path.to_path_buf())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_marked_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FIXTURE_MARKER), "").unwrap();
        assert_eq!(FixtureRoot::new(dir.path()).path(), dir.path());
    }

    #[test]
    #[should_panic(expected = "SECURITY")]
    fn rejects_unmarked_directory() {
        let dir = tempfile::tempdir().unwrap();
        FixtureRoot::new(dir.path());
    }
}
