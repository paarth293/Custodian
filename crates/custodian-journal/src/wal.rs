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

/// Upper bound for a single record; a larger length prefix is treated as corruption
/// instead of being allocated.
const MAX_RECORD_LEN: u32 = 16 * 1024 * 1024;

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

        let mut journal = Self { file };
        // A crash can leave a torn record at the tail. Cut it off now, otherwise
        // every record appended after it would be unreachable by the reader.
        let good_len = journal.scan()?.1;
        if good_len < journal.file.metadata()?.len() {
            journal.file.set_len(good_len)?;
        }
        Ok(journal)
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
        Ok(self.scan()?.0)
    }

    /// Reads records from the start until EOF, a torn write or corruption.
    /// Returns them with the byte length of the valid prefix.
    fn scan(&mut self) -> Result<(Vec<Vec<u8>>, u64), JournalError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut records = Vec::new();
        let mut good_len = 0u64;

        loop {
            let mut len_buf = [0u8; 4];
            if self.file.read_exact(&mut len_buf).is_err() {
                break; // EOF or torn write
            }
            let len = u32::from_le_bytes(len_buf);
            if len > MAX_RECORD_LEN {
                break; // corrupt length prefix
            }

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

            good_len += 8 + u64::from(len);
            records.push(data);
        }

        Ok((records, good_len))
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
    fn test_records_after_torn_tail_are_readable() {
        let path = std::env::temp_dir().join("test_journal_torn_append.log");
        let _ = fs::remove_file(&path);

        {
            let mut j = Journal::new(&path).unwrap();
            j.write_record(b"good1").unwrap();
            j.file.write_all(&[10, 0, 0, 0, 0, 0, 0, 0]).unwrap(); // header only
            j.file.write_all(b"bad").unwrap(); // torn payload
        }
        {
            let mut j = Journal::new(&path).unwrap();
            j.write_record(b"good2").unwrap();
        }

        let mut j = Journal::new(&path).unwrap();
        let records = j.read_all_good_records().unwrap();
        assert_eq!(records, vec![b"good1".to_vec(), b"good2".to_vec()]);
    }

    #[test]
    fn test_oversized_length_prefix_is_corruption() {
        let path = std::env::temp_dir().join("test_journal_huge_len.log");
        let _ = fs::remove_file(&path);
        fs::write(&path, [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]).unwrap();

        let mut j = Journal::new(&path).unwrap();
        assert!(j.read_all_good_records().unwrap().is_empty());
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
