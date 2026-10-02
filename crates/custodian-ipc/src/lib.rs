use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use hmac::{Hmac, Mac};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Sha256;
use std::io::{self, Read, Write};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

const MAGIC: [u8; 4] = *b"CUST";
const MAX_FRAME_SIZE: u32 = 10 * 1024 * 1024; // 10 MB

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid magic bytes")]
    InvalidMagic,
    #[error("Frame too large: {0}")]
    FrameTooLarge(u32),
    #[error("Invalid HMAC signature")]
    InvalidSignature,
}

pub fn write_frame<T: Serialize, W: Write>(
    writer: &mut W,
    payload: &T,
    key: &[u8],
) -> Result<(), IpcError> {
    let json_bytes = serde_json::to_vec(payload)?;
    let len = json_bytes.len() as u32;

    if len > MAX_FRAME_SIZE {
        return Err(IpcError::FrameTooLarge(len));
    }

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(&json_bytes);
    let signature = mac.finalize().into_bytes();

    writer.write_all(&MAGIC)?;
    writer.write_u32::<LittleEndian>(len)?;
    writer.write_all(&signature)?;
    writer.write_all(&json_bytes)?;
    writer.flush()?;

    Ok(())
}

pub fn read_frame<T: DeserializeOwned, R: Read>(
    reader: &mut R,
    key: &[u8],
) -> Result<T, IpcError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(IpcError::InvalidMagic);
    }

    let len = reader.read_u32::<LittleEndian>()?;
    if len > MAX_FRAME_SIZE {
        return Err(IpcError::FrameTooLarge(len));
    }

    let mut expected_sig = [0u8; 32];
    reader.read_exact(&mut expected_sig)?;

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(&payload);
    if mac.verify_slice(&expected_sig).is_err() {
        return Err(IpcError::InvalidSignature);
    }

    let item: T = serde_json::from_slice(&payload)?;
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::io::Cursor;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Dummy {
        data: String,
    }

    #[test]
    fn test_round_trip() {
        let key = b"super-secret";
        let payload = Dummy {
            data: "hello world".to_string(),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &payload, key).unwrap();

        let mut cursor = Cursor::new(buf);
        let decoded: Dummy = read_frame(&mut cursor, key).unwrap();

        assert_eq!(payload, decoded);
    }

    #[test]
    fn test_invalid_mac() {
        let key1 = b"secret1";
        let key2 = b"secret2";
        let payload = Dummy {
            data: "hello".to_string(),
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &payload, key1).unwrap();

        let mut cursor = Cursor::new(buf);
        let result: Result<Dummy, _> = read_frame(&mut cursor, key2);
        assert!(matches!(result, Err(IpcError::InvalidSignature)));
    }
}
