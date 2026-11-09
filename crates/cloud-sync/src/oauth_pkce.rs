//! PKCE (RFC 7636): a 32-byte random verifier (43 base64url chars), its S256
//! challenge, and a random `state` that ties the browser redirect to this sign-in.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::secret_string::SecretString;

/// Verifier (secret until the token exchange) + S256 challenge (sent to the browser).
#[derive(Debug, Clone)]
pub struct PkcePair {
    pub verifier: SecretString,
    pub challenge: String,
}

impl PkcePair {
    /// Fresh pair from the OS RNG.
    pub fn generate() -> std::io::Result<Self> {
        let verifier = random_base64url::<32>()?;
        Ok(Self {
            challenge: s256_challenge(&verifier),
            verifier: SecretString::new(verifier),
        })
    }
}

/// `BASE64URL(SHA256(verifier))` without padding.
pub fn s256_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Random `state` (128 bits, base64url).
pub fn random_state() -> std::io::Result<String> {
    random_base64url::<16>()
}

fn random_base64url<const N: usize>() -> std::io::Result<String> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        assert_eq!(
            s256_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_pairs_are_random_and_consistent() {
        let a = PkcePair::generate().unwrap();
        let b = PkcePair::generate().unwrap();
        assert_eq!(a.verifier.expose().len(), 43);
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.challenge, s256_challenge(a.verifier.expose()));
        assert_eq!(random_state().unwrap().len(), 22);
        assert!(!format!("{a:?}").contains(a.verifier.expose()));
    }
}
