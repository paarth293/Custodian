use crc32fast::Hasher as Crc32;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

type Hash = [u8; 32];

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("tamper detected")]
    TamperDetected,
    #[error("record too large")]
    RecordTooLarge,
}

/// Append-only ledger of `[sha256(prev || data)][crc32(data)][len: u32][data]`
/// records. Changing, removing or truncating any record breaks the chain.
pub struct Ledger {
    file: File,
    last_hash: Hash,
}

struct Record {
    hash: Hash,
    crc: u32,
    data: Vec<u8>,
}

impl Ledger {
    /// Opens or creates a ledger and resumes the chain from its last record.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        let mut ledger = Self {
            file,
            last_hash: [0u8; 32],
        };
        ledger.file.seek(SeekFrom::Start(0))?;
        while let Some(record) = ledger.read_record()? {
            ledger.last_hash = record.hash;
        }
        Ok(ledger)
    }

    pub fn append_record(&mut self, data: &[u8]) -> Result<(), LedgerError> {
        let len = u32::try_from(data.len()).map_err(|_| LedgerError::RecordTooLarge)?;
        let hash = chain_hash(&self.last_hash, data);

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&hash)?;
        self.file.write_all(&crc32(data).to_le_bytes())?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(data)?;
        self.file.sync_all()?;

        self.last_hash = hash;
        Ok(())
    }

    /// Re-validates every record from the start of the file.
    pub fn verify(&mut self) -> Result<(), LedgerError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut prev = [0u8; 32];

        while let Some(record) = self.read_record()? {
            if crc32(&record.data) != record.crc || chain_hash(&prev, &record.data) != record.hash {
                return Err(LedgerError::TamperDetected);
            }
            prev = record.hash;
        }
        Ok(())
    }

    /// Reads the next record; `Ok(None)` at a clean end of file and
    /// `TamperDetected` for a truncated record.
    fn read_record(&mut self) -> Result<Option<Record>, LedgerError> {
        let mut header = [0u8; 40];
        let read = read_up_to(&mut self.file, &mut header)?;
        if read == 0 {
            return Ok(None);
        }
        if read < header.len() {
            return Err(LedgerError::TamperDetected);
        }

        let hash: Hash = header[..32].try_into().unwrap();
        let crc = u32::from_le_bytes(header[32..36].try_into().unwrap());
        let len = u32::from_le_bytes(header[36..40].try_into().unwrap());

        let mut data = Vec::new();
        (&mut self.file)
            .take(u64::from(len))
            .read_to_end(&mut data)?;
        if data.len() != len as usize {
            return Err(LedgerError::TamperDetected);
        }
        Ok(Some(Record { hash, crc, data }))
    }
}

fn chain_hash(prev: &Hash, data: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(prev);
    hasher.update(data);
    hasher.finalize().into()
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(data);
    crc.finalize()
}

fn read_up_to(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_modified_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.bin");

        {
            let mut ledger = Ledger::new(&path).unwrap();
            ledger.append_record(b"tx1").unwrap();
            ledger.append_record(b"tx2").unwrap();
            ledger.verify().unwrap();
        }

        {
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(40)).unwrap();
            file.write_all(&[0xFF]).unwrap();
        }

        let mut ledger = Ledger::new(&path).unwrap();
        assert!(matches!(ledger.verify(), Err(LedgerError::TamperDetected)));
    }

    #[test]
    fn chain_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.bin");

        Ledger::new(&path).unwrap().append_record(b"tx1").unwrap();
        let mut ledger = Ledger::new(&path).unwrap();
        ledger.append_record(b"tx2").unwrap();
        ledger.verify().unwrap();
    }

    #[test]
    fn detects_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.bin");
        {
            let mut ledger = Ledger::new(&path).unwrap();
            ledger.append_record(b"tx1").unwrap();
        }
        let len = std::fs::metadata(&path).unwrap().len();
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(len - 1)
            .unwrap();

        assert!(matches!(
            Ledger::new(&path),
            Err(LedgerError::TamperDetected)
        ));
    }
}
