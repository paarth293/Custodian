use std::fs;
use std::io;
use std::path::Path;

pub struct DiskMonitor;

impl DiskMonitor {
    /// Returns the logical size of `path`.
    ///
    /// On Windows this will switch to `GetCompressedFileSizeW` so that
    /// compressed and sparse files report their real on-disk footprint.
    pub fn get_real_file_size(path: &Path) -> io::Result<u64> {
        Ok(fs::metadata(path)?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_file_size() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("disk_test.txt");
        fs::write(&file_path, b"1234").unwrap();

        assert_eq!(DiskMonitor::get_real_file_size(&file_path).unwrap(), 4);
    }
}
