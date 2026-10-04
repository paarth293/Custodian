use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

/// Journals larger than this are refused so a runaway writer cannot fill the disk.
pub const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("E_SPACE: journal exceeds {MAX_JOURNAL_BYTES} bytes")]
    Space,
    #[error("record too large")]
    RecordTooLarge,
}

/// Append-only journal of `[len: u32][crc32: u32][data]` records.
///
/// A crash mid-write leaves a torn or corrupt record at the end. Opening the
/// journal truncates everything from the first bad record onwards so new
/// records are appended directly after the last good one (and stay readable).
pub struct Journal {
    file: File,
    recovered_bytes: u64,
}

impl Journal {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, JournalError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        let len = file.metadata()?.len();
        if len > MAX_JOURNAL_BYTES {
            return Err(JournalError::Space);
        }

        let mut journal = Self {
            file,
            recovered_bytes: 0,
        };
        let (_, good_len) = journal.scan()?;
        if good_len < len {
            journal.file.set_len(good_len)?;
            journal.file.sync_all()?;
            journal.recovered_bytes = len - good_len;
        }
        Ok(journal)
    }

    /// Bytes of torn or corrupt records discarded when the journal was opened.
    pub fn recovered_bytes(&self) -> u64 {
        self.recovered_bytes
    }

    /// Appends a record and flushes it to stable storage.
    pub fn write_record(&mut self, data: &[u8]) -> Result<(), JournalError> {
        let len = u32::try_from(data.len()).map_err(|_| JournalError::RecordTooLarge)?;

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&crc32(data).to_le_bytes())?;
        self.file.write_all(data)?;
        self.file.sync_all()?;
        Ok(())
    }

    /// Returns every intact record from the start of the journal.
    pub fn read_all_good_records(&mut self) -> Result<Vec<Vec<u8>>, JournalError> {
        Ok(self.scan()?.0)
    }

    /// Reads records until the first torn or corrupt one; returns them and the
    /// byte length of the intact prefix.
    fn scan(&mut self) -> Result<(Vec<Vec<u8>>, u64), JournalError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut records = Vec::new();
        let mut good_len = 0u64;

        loop {
            let mut header = [0u8; 8];
            if self.file.read_exact(&mut header).is_err() {
                break;
            }
            let len = u32::from_le_bytes(header[..4].try_into().unwrap());
            let expected_crc = u32::from_le_bytes(header[4..].try_into().unwrap());

            let mut data = Vec::new();
            let read = (&mut self.file)
                .take(u64::from(len))
                .read_to_end(&mut data)?;
            if read != len as usize || crc32(&data) != expected_crc {
                break;
            }
            good_len += 8 + u64::from(len);
            records.push(data);
        }

        Ok((records, good_len))
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_from_torn_write_and_keeps_appending() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.log");

        {
            let mut j = Journal::new(&path).unwrap();
            j.write_record(b"good1").unwrap();
            j.write_record(b"good2").unwrap();

            // Simulate a crash mid-record: header claims 10 bytes, only 3 written.
            j.file.write_all(&10u32.to_le_bytes()).unwrap();
            j.file.write_all(&0u32.to_le_bytes()).unwrap();
            j.file.write_all(b"bad").unwrap();
        }

        let mut j = Journal::new(&path).unwrap();
        assert_eq!(j.recovered_bytes(), 11);
        j.write_record(b"tx3").unwrap();

        let records = Journal::new(&path)
            .unwrap()
            .read_all_good_records()
            .unwrap();
        assert_eq!(
            records,
            vec![b"good1".to_vec(), b"good2".to_vec(), b"tx3".to_vec()]
        );
    }

    #[test]
    fn refuses_oversized_journal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("full.log");
        File::create(&path)
            .unwrap()
            .set_len(MAX_JOURNAL_BYTES + 1)
            .unwrap();

        assert!(matches!(Journal::new(&path), Err(JournalError::Space)));
    }
}
