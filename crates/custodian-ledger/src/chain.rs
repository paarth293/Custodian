use sha2::{Sha256, Digest};
use crc32fast::Hasher;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Tamper detected")]
    TamperDetected,
}

pub struct Ledger {
    file: File,
    last_hash: [u8; 32],
}

impl Ledger {
    pub fn new(path: &str) -> Result<Self, LedgerError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
            
        Ok(Self {
            file,
            last_hash: [0u8; 32],
        })
    }

    pub fn append_record(&mut self, data: &[u8]) -> Result<(), LedgerError> {
        let mut hasher = Sha256::new();
        hasher.update(self.last_hash);
        hasher.update(data);
        let new_hash = hasher.finalize();

        let mut crc = Hasher::new();
        crc.update(data);
        let crc_val = crc.finalize();

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&new_hash)?;
        self.file.write_all(&crc_val.to_le_bytes())?;
        self.file.write_all(&(data.len() as u32).to_le_bytes())?;
        self.file.write_all(data)?;
        
        self.last_hash.copy_from_slice(&new_hash);
        
        Ok(())
    }

    pub fn verify(&mut self) -> Result<(), LedgerError> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut prev_hash = [0u8; 32];
        
        loop {
            let mut expected_hash = [0u8; 32];
            if self.file.read_exact(&mut expected_hash).is_err() {
                break;
            }
            
            let mut crc_buf = [0u8; 4];
            self.file.read_exact(&mut crc_buf)?;
            let expected_crc = u32::from_le_bytes(crc_buf);
            
            let mut len_buf = [0u8; 4];
            self.file.read_exact(&mut len_buf)?;
            let len = u32::from_le_bytes(len_buf);
            
            let mut data = vec![0u8; len as usize];
            self.file.read_exact(&mut data)?;
            
            let mut crc = Hasher::new();
            crc.update(&data);
            if crc.finalize() != expected_crc {
                return Err(LedgerError::TamperDetected);
            }
            
            let mut hasher = Sha256::new();
            hasher.update(prev_hash);
            hasher.update(&data);
            if hasher.finalize().as_slice() != expected_hash {
                return Err(LedgerError::TamperDetected);
            }
            
            prev_hash.copy_from_slice(&expected_hash);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_t_k1_tamper_detection() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("ledger_test.bin");
        let _ = fs::remove_file(&path);
        
        {
            let mut ledger = Ledger::new(path.to_str().unwrap()).unwrap();
            ledger.append_record(b"tx1").unwrap();
            ledger.append_record(b"tx2").unwrap();
            ledger.verify().unwrap();
        }
        
        {
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(40)).unwrap();
            file.write_all(&[0xFF]).unwrap();
        }
        
        let mut ledger = Ledger::new(path.to_str().unwrap()).unwrap();
        assert!(matches!(ledger.verify(), Err(LedgerError::TamperDetected)));
    }
}
