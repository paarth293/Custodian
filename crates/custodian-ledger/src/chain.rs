use crc32fast::Hasher as Crc32;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

type Hash = [u8; 32];

const HEADER_LEN: usize = 32 + 4 + 4;

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
/// records.
///
/// Modifying any complete record breaks the chain and fails [`Ledger::verify`].
/// An incomplete *trailing* record is what a crash during
/// [`Ledger::append_record`] leaves behind, so opening the ledger truncates it
/// and reports it through [`Ledger::recovered_bytes`]. Removing whole records
/// from the end is only detectable against an external anchor
/// (see [`crate::anchor`]).
pub struct Ledger {
    file: File,
    last_hash: Hash,
    recovered_bytes: u64,
}

struct Record {
    hash: Hash,
    crc: u32,
    data: Vec<u8>,
}

enum ReadOutcome {
    Record(Record),
    /// Clean end of file.
    End,
    /// The file ends part-way through a record.
    Torn,
}

impl Ledger {
    /// Opens or creates a ledger, discards a torn trailing record left by a
    /// crash, and resumes the chain from the last complete record.
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
            recovered_bytes: 0,
        };

        ledger.file.seek(SeekFrom::Start(0))?;
        let mut good_len = 0;
        loop {
            match ledger.read_record()? {
                ReadOutcome::Record(record) => {
                    ledger.last_hash = record.hash;
                    good_len = ledger.file.stream_position()?;
                }
                ReadOutcome::End => break,
                ReadOutcome::Torn => {
                    let len = ledger.file.metadata()?.len();
                    ledger.file.set_len(good_len)?;
                    ledger.file.sync_all()?;
                    ledger.recovered_bytes = len - good_len;
                    break;
                }
            }
        }
        Ok(ledger)
    }

    /// Bytes of a torn trailing record discarded when the ledger was opened.
    pub fn recovered_bytes(&self) -> u64 {
        self.recovered_bytes
    }

    pub fn append_record(&mut self, data: &[u8]) -> Result<(), LedgerError> {
        let len = u32::try_from(data.len()).map_err(|_| LedgerError::RecordTooLarge)?;
        let hash = chain_hash(&self.last_hash, data);

        let mut record = Vec::with_capacity(HEADER_LEN + data.len());
        record.extend_from_slice(&hash);
        record.extend_from_slice(&crc32(data).to_le_bytes());
        record.extend_from_slice(&len.to_le_bytes());
        record.extend_from_slice(data);

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&record)?;
        self.file.sync_all()?;

        self.last_hash = hash;
        Ok(())
    }

    /// Re-validates every record from the start of the file.
    pub fn verify(&mut self) -> Result<(), LedgerError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut prev = [0u8; 32];

        loop {
            match self.read_record()? {
                ReadOutcome::Record(record) => {
                    if crc32(&record.data) != record.crc
                        || chain_hash(&prev, &record.data) != record.hash
                    {
                        return Err(LedgerError::TamperDetected);
                    }
                    prev = record.hash;
                }
                ReadOutcome::End => return Ok(()),
                ReadOutcome::Torn => return Err(LedgerError::TamperDetected),
            }
        }
    }

    fn read_record(&mut self) -> Result<ReadOutcome, LedgerError> {
        let mut header = [0u8; HEADER_LEN];
        let read = read_up_to(&mut self.file, &mut header)?;
        if read == 0 {
            return Ok(ReadOutcome::End);
        }
        if read < HEADER_LEN {
            return Ok(ReadOutcome::Torn);
        }

        let hash: Hash = header[..32].try_into().unwrap();
        let crc = u32::from_le_bytes(header[32..36].try_into().unwrap());
        let len = u32::from_le_bytes(header[36..40].try_into().unwrap());

        let mut data = Vec::new();
        (&mut self.file)
            .take(u64::from(len))
            .read_to_end(&mut data)?;
        if data.len() != len as usize {
            return Ok(ReadOutcome::Torn);
        }
        Ok(ReadOutcome::Record(Record { hash, crc, data }))
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
    use std::fs;

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
    fn recovers_from_torn_append() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.bin");
        {
            let mut ledger = Ledger::new(&path).unwrap();
            ledger.append_record(b"tx1").unwrap();
            ledger.append_record(b"tx2").unwrap();
        }
        let complete_len = fs::metadata(&path).unwrap().len();

        // Crash after the hash was written but before the rest of the record.
        let partial = [0xAB; 32];
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&partial)
            .unwrap();

        let mut ledger = Ledger::new(&path).unwrap();
        assert_eq!(ledger.recovered_bytes(), partial.len() as u64);
        assert_eq!(fs::metadata(&path).unwrap().len(), complete_len);

        ledger.append_record(b"tx3").unwrap();
        ledger.verify().unwrap();
    }

    #[test]
    fn verify_rejects_truncation_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.bin");
        let mut ledger = Ledger::new(&path).unwrap();
        ledger.append_record(b"tx1").unwrap();

        let len = fs::metadata(&path).unwrap().len();
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(len - 1)
            .unwrap();

        assert!(matches!(ledger.verify(), Err(LedgerError::TamperDetected)));
    }
}
