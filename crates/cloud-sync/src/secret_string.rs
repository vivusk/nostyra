//! A token / verifier / client secret that never reaches logs: `Debug` prints
//! `<redacted>`, there is no `Display`, the value is only read through
//! [`SecretString::expose`] at the one place it goes on the wire or to disk.

/// Secret text with a redacted `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw value (request header / form body / encrypted file only).
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let secret = SecretString::new("1//refresh-abc");
        assert_eq!(format!("{secret:?}"), "<redacted>");
        assert_eq!(secret.expose(), "1//refresh-abc");
    }
}
