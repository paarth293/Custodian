//! Authenticated IPC framing shared by every Custodian process.
//!
//! Wire format: `"CUST" | len: u32 LE | HMAC-SHA256(payload): 32 bytes | payload`,
//! where `payload` is UTF-8 JSON of at most [`MAX_FRAME_SIZE`] bytes. The
//! TypeScript (`services/agent/src/ipc.ts`) and Python
//! (`services/mlhost/mlhost/ipc.py`) implementations are byte-compatible.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{de::DeserializeOwned, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

pub const MAGIC: [u8; 4] = *b"CUST";
pub const MAX_FRAME_SIZE: u32 = 10 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid magic bytes")]
    InvalidMagic,
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("invalid HMAC signature")]
    InvalidSignature,
}

fn mac(key: &[u8]) -> HmacSha256 {
    HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length")
}

pub fn write_frame<T: Serialize, W: Write>(
    writer: &mut W,
    payload: &T,
    key: &[u8],
) -> Result<(), IpcError> {
    let json = serde_json::to_vec(payload)?;
    let len = u32::try_from(json.len())
        .ok()
        .filter(|&len| len <= MAX_FRAME_SIZE)
        .ok_or(IpcError::FrameTooLarge(json.len()))?;

    let mut mac = mac(key);
    mac.update(&json);

    writer.write_all(&MAGIC)?;
    writer.write_u32::<LittleEndian>(len)?;
    writer.write_all(&mac.finalize().into_bytes())?;
    writer.write_all(&json)?;
    writer.flush()?;
    Ok(())
}

pub fn read_frame<T: DeserializeOwned, R: Read>(reader: &mut R, key: &[u8]) -> Result<T, IpcError> {
    read_frame_limited(reader, key, MAX_FRAME_SIZE)
}

/// A narrower peer-specific ceiling, checked before allocating payload memory.
pub fn read_frame_limited<T: DeserializeOwned, R: Read>(
    reader: &mut R,
    key: &[u8],
    maximum: u32,
) -> Result<T, IpcError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(IpcError::InvalidMagic);
    }

    let len = reader.read_u32::<LittleEndian>()?;
    if len > maximum.min(MAX_FRAME_SIZE) {
        return Err(IpcError::FrameTooLarge(len as usize));
    }

    let mut signature = [0u8; 32];
    reader.read_exact(&mut signature)?;
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    let mut mac = mac(key);
    mac.update(&payload);
    mac.verify_slice(&signature)
        .map_err(|_| IpcError::InvalidSignature)?;

    Ok(serde_json::from_slice(&payload)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Supervisor,
    Core,
    Engine,
    Agent,
    MlHost,
    Orchestrator,
    Broker,
    Relay,
    Sandbox,
}

/// Per-role HMAC keys, generated fresh from the OS CSPRNG on every launch.
#[derive(Default)]
pub struct PeerMatrix {
    keys: HashMap<Role, [u8; 32]>,
}

impl PeerMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates (or rotates) the key for `role` and returns it.
    pub fn generate_key(&mut self, role: Role) -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        self.keys.insert(role, key);
        key
    }

    pub fn get_key(&self, role: Role) -> Option<[u8; 32]> {
        self.keys.get(&role).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::io::Cursor;

    #[test]
    fn narrower_peer_limit_is_enforced_before_payload_read() {
        let mut header = MAGIC.to_vec();
        header.extend_from_slice(&1024u32.to_le_bytes());
        let result: Result<serde_json::Value, _> =
            read_frame_limited(&mut Cursor::new(header), b"key", 128);
        assert!(matches!(result, Err(IpcError::FrameTooLarge(1024))));
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Dummy {
        data: String,
    }

    fn encode(payload: &Dummy, key: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        write_frame(&mut buf, payload, key).unwrap();
        buf
    }

    #[test]
    fn round_trip() {
        let payload = Dummy {
            data: "hello world".into(),
        };
        let decoded: Dummy = read_frame(&mut Cursor::new(encode(&payload, b"k")), b"k").unwrap();
        assert_eq!(payload, decoded);
    }

    #[test]
    fn rejects_wrong_key() {
        let buf = encode(
            &Dummy {
                data: "hello".into(),
            },
            b"secret1",
        );
        let result: Result<Dummy, _> = read_frame(&mut Cursor::new(buf), b"secret2");
        assert!(matches!(result, Err(IpcError::InvalidSignature)));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut buf = encode(&Dummy { data: "x".into() }, b"k");
        buf[0] = b'X';
        let result: Result<Dummy, _> = read_frame(&mut Cursor::new(buf), b"k");
        assert!(matches!(result, Err(IpcError::InvalidMagic)));
    }

    #[test]
    fn rejects_oversized_length() {
        let mut buf = MAGIC.to_vec();
        buf.extend_from_slice(&(MAX_FRAME_SIZE + 1).to_le_bytes());
        let result: Result<Dummy, _> = read_frame(&mut Cursor::new(buf), b"k");
        assert!(matches!(result, Err(IpcError::FrameTooLarge(_))));
    }

    #[test]
    fn peer_keys_are_random_and_stored() {
        let mut matrix = PeerMatrix::new();
        let core = matrix.generate_key(Role::Core);
        let engine = matrix.generate_key(Role::Engine);
        assert_ne!(core, engine);
        assert_eq!(matrix.get_key(Role::Core), Some(core));
        assert_eq!(matrix.get_key(Role::Agent), None);
    }
}
