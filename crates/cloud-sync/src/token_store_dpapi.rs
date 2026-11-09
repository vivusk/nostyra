//! Windows DPAPI for the refresh-token file: `CryptProtectData` /
//! `CryptUnprotectData` with the current user's key and a fixed entropy, so the file
//! is readable only by this Windows account on this PC (a copied data folder → "Sign
//! in again"). The only `unsafe` code of the crate.

use std::io;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Extra secret mixed into the key (versioned: a format change gets a new value).
const ENTROPY: &[u8] = b"nostyra-cloud-v1";

fn blob(bytes: &[u8]) -> io::Result<CRYPT_INTEGER_BLOB> {
    Ok(CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| io::Error::other("token too large"))?,
        // DPAPI only reads input blobs; the API type is `*mut` for historical reasons.
        pbData: bytes.as_ptr().cast_mut(),
    })
}

/// Encrypts `plain` for the current Windows user.
#[allow(unsafe_code)]
pub fn protect(plain: &[u8]) -> io::Result<Vec<u8>> {
    let input = blob(plain)?;
    let entropy = blob(ENTROPY)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    // SAFETY: `input` / `entropy` point to live slices of the stated lengths for the
    // whole call; description, reserved and prompt pointers may be null; `output` is
    // a valid out-pointer DPAPI fills with a LocalAlloc'd buffer on success.
    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            &entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success `output.pbData` holds `output.cbData` initialized bytes owned
    // by us; they are copied out, then freed exactly once with `LocalFree`.
    let sealed = unsafe {
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(sealed)
}

/// Decrypts a [`protect`] result; fails for another user / PC or a tampered file.
#[allow(unsafe_code)]
pub fn unprotect(sealed: &[u8]) -> io::Result<Vec<u8>> {
    let input = blob(sealed)?;
    let entropy = blob(ENTROPY)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    // SAFETY: as in `protect`; the description out-pointer is null (not requested).
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            &entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: as in `protect`: copy the LocalAlloc'd plaintext, free it once.
    let plain = unsafe {
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        bytes
    };
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let sealed = protect(b"1//refresh-token").unwrap();
        assert_ne!(&sealed[..], b"1//refresh-token");
        assert_eq!(unprotect(&sealed).unwrap(), b"1//refresh-token");
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x55;
        assert!(unprotect(&tampered).is_err());
        assert!(unprotect(b"not dpapi").is_err());
    }
}
