//! Data protection for the current Windows user (DPAPI, SR-A3).
//!
//! `protect` encrypts and authenticates bytes with a key derived from the
//! user's logon secret, so the result can be read back only by the same user
//! on the same machine. Tampered input fails `unprotect` instead of decrypting
//! to garbage.

use std::io;

/// Application entropy: other programs running as the same user cannot
/// unprotect Custodian's blobs without also knowing this value.
#[cfg_attr(not(windows), allow(dead_code))]
const ENTROPY: &[u8] = b"custodian-dpapi-v1";

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::ENTROPY;
    use std::{io, ptr, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn blob(data: &[u8]) -> io::Result<CRYPT_INTEGER_BLOB> {
        Ok(CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(data.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "data too large"))?,
            // DPAPI never writes through the input pointers.
            pbData: data.as_ptr().cast_mut(),
        })
    }

    /// Copies a DPAPI output blob into a `Vec` and frees the system buffer.
    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if out.pbData.is_null() {
            return Vec::new();
        }
        // SAFETY: on success DPAPI returns `cbData` initialised bytes at
        // `pbData`, allocated with LocalAlloc and owned by us until freed.
        let bytes = unsafe { slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        // SAFETY: `pbData` came from DPAPI's LocalAlloc and is freed exactly once.
        unsafe { LocalFree(out.pbData.cast()) };
        bytes
    }

    pub fn protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let input = blob(plaintext)?;
        let entropy = blob(ENTROPY)?;
        let mut out = CRYPT_INTEGER_BLOB::default();
        // SAFETY: every pointer refers to a live blob for the whole call; the
        // description, reserved and prompt arguments may be null.
        let ok = unsafe {
            CryptProtectData(
                &input,
                ptr::null(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(take(out))
    }

    pub fn unprotect(ciphertext: &[u8]) -> io::Result<Vec<u8>> {
        let input = blob(ciphertext)?;
        let entropy = blob(ENTROPY)?;
        let mut out = CRYPT_INTEGER_BLOB::default();
        // SAFETY: as in `protect`; the description out-pointer may be null.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(take(out))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "DPAPI is available only on Windows",
        )
    }

    pub fn protect(_: &[u8]) -> io::Result<Vec<u8>> {
        Err(unsupported())
    }

    pub fn unprotect(_: &[u8]) -> io::Result<Vec<u8>> {
        Err(unsupported())
    }
}

/// Encrypts `plaintext` for the current user (`CryptProtectData`).
pub fn protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
    imp::protect(plaintext)
}

/// Decrypts a blob from [`protect`]; fails if it was altered or belongs to
/// another user (`CryptUnprotectData`).
pub fn unprotect(ciphertext: &[u8]) -> io::Result<Vec<u8>> {
    imp::unprotect(ciphertext)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_hides_the_plaintext() {
        let secret = b"secret-key-material-0123456789";
        let sealed = protect(secret).unwrap();
        assert!(sealed.len() > secret.len());
        assert!(!sealed.windows(secret.len()).any(|w| w == secret));
        assert_eq!(unprotect(&sealed).unwrap(), secret);
    }

    #[test]
    fn empty_input_roundtrips() {
        assert_eq!(unprotect(&protect(b"").unwrap()).unwrap(), b"");
    }

    #[test]
    fn tampered_blob_is_refused() {
        let mut sealed = protect(b"approval key").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(unprotect(&sealed).is_err());
    }

    #[test]
    fn same_plaintext_encrypts_differently() {
        assert_ne!(protect(b"x").unwrap(), protect(b"x").unwrap());
    }
}
