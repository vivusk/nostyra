//! Random identifiers from the OS RNG, plus the short hash used in log lines.

use sha2::{Digest, Sha256};

/// `bytes` random bytes as lowercase hex (room id: 16, token peer id: 8).
pub fn random_hex(bytes: usize) -> anyhow::Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| anyhow::anyhow!("OS random source failed: {e}"))?;
    Ok(hex(&buf))
}

/// First 8 hex chars of sha256(`value`): lets log lines of one room be grouped
/// without writing the room id (or anything else secret) itself.
pub fn log_hash(value: &str) -> String {
    hex(&Sha256::digest(value.as_bytes())[..4])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_ids_have_the_requested_size() {
        let id = random_hex(16).unwrap();
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(id, random_hex(16).unwrap());
    }

    #[test]
    fn log_hash_is_short_and_stable() {
        assert_eq!(log_hash("abc"), "ba7816bf");
        assert_eq!(log_hash("abc").len(), 8);
    }
}
