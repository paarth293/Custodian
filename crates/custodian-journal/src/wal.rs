use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("E_SPACE")]
    Space,
    #[error("Corrupt record")]
    Corrupt,
}

pub struct Journal {
    file: File,
}

impl Journal {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, JournalError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
            
        // Mock disk full check
        let meta = file.metadata()?;
        if meta.len() > 64 * 1024 * 1024 {
            return Err(JournalError::Space);
        }

        Ok(Self { file })
    }

    pub fn write_record(&mut self, data: &[u8]) -> Result<(), JournalError> {
        let mut hasher = Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();

        let len = data.len() as u32;
        
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&crc.to_le_bytes())?;
        self.file.write_all(data)?;
        
        // Mock FlushFileBuffers
        self.file.sync_all()?;
        
        Ok(())
    }

    pub fn read_all_good_records(&mut self) -> Result<Vec<Vec<u8>>, JournalError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut records = Vec::new();

        loop {
            let mut len_buf = [0u8; 4];
            if self.file.read_exact(&mut len_buf).is_err() {
                break; // EOF or torn write
            }
            let len = u32::from_le_bytes(len_buf);

            let mut crc_buf = [0u8; 4];
            if self.file.read_exact(&mut crc_buf).is_err() {
                break; // torn write
            }
            let expected_crc = u32::from_le_bytes(crc_buf);

            let mut data = vec![0u8; len as usize];
            if self.file.read_exact(&mut data).is_err() {
                break; // torn write
            }

            let mut hasher = Hasher::new();
            hasher.update(&data);
            if hasher.finalize() != expected_crc {
                break; // corrupt
            }

            records.push(data);
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_torn_write_recovery() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_journal.log");
        let _ = fs::remove_file(&path);
        
        {
            let mut j = Journal::new(&path).unwrap();
            j.write_record(b"good1").unwrap();
            j.write_record(b"good2").unwrap();
            
            // torn write
            j.file.write_all(&[10, 0, 0, 0]).unwrap(); // len=10
            j.file.write_all(&[0, 0, 0, 0]).unwrap(); // crc
            j.file.write_all(b"bad").unwrap(); // not 10 bytes
        }
        
        let mut j = Journal::new(&path).unwrap();
        let records = j.read_all_good_records().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], b"good1");
        assert_eq!(records[1], b"good2");
    }
    
    #[test]
    fn test_space_limit() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_full.log");
        let file = File::create(&path).unwrap();
        file.set_len(65 * 1024 * 1024).unwrap(); // > 64MB
        
        let res = Journal::new(&path);
        assert!(matches!(res, Err(JournalError::Space)));
    }
}
