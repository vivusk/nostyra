//! Room codes: 6 characters of Crockford base32 (no I, L, O, U), typed by people.

/// Crockford base32 alphabet: digits + letters without I, L, O, U (no look-alikes).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Length of a room code.
pub const ROOM_CODE_LEN: usize = 6;

/// A fresh random room code (OS RNG). 32 divides 256, so `byte % 32` is unbiased.
pub fn generate_room_code() -> anyhow::Result<String> {
    let mut bytes = [0u8; ROOM_CODE_LEN];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("OS random source failed: {e}"))?;
    Ok(bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b % 32)]))
        .collect())
}

/// Normalises what a person typed: drops spaces / dashes, uppercases and maps the
/// look-alikes (`O` → `0`, `I` / `L` → `1`). `None` when the result is not a valid code.
pub fn normalize_room_code(input: &str) -> Option<String> {
    let mut code = String::with_capacity(ROOM_CODE_LEN);
    for c in input.chars() {
        if c == ' ' || c == '-' {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        if !c.is_ascii() || !ALPHABET.contains(&(c as u8)) || code.len() == ROOM_CODE_LEN {
            return None;
        }
        code.push(c);
    }
    (code.len() == ROOM_CODE_LEN).then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_are_valid_and_vary() {
        let a = generate_room_code().unwrap();
        assert_eq!(a.len(), ROOM_CODE_LEN);
        assert_eq!(normalize_room_code(&a).as_deref(), Some(a.as_str()));
        assert!(!a.contains(['I', 'L', 'O', 'U']));
        let distinct: std::collections::HashSet<_> =
            (0..50).map(|_| generate_room_code().unwrap()).collect();
        assert!(distinct.len() > 45, "codes should be random");
    }

    #[test]
    fn normalisation_maps_look_alikes_and_case() {
        assert_eq!(normalize_room_code("ab-c d1e").as_deref(), Some("ABCD1E"));
        assert_eq!(normalize_room_code("oIlxyz").as_deref(), Some("011XYZ"));
        assert_eq!(normalize_room_code(" 7k9-m2p ").as_deref(), Some("7K9M2P"));
    }

    #[test]
    fn normalisation_rejects_bad_input() {
        assert_eq!(normalize_room_code("ABCDE"), None);
        assert_eq!(normalize_room_code("ABCDEFG"), None);
        assert_eq!(
            normalize_room_code("ABCDEU"),
            None,
            "U is not in the alphabet"
        );
        assert_eq!(normalize_room_code("ABC/EF"), None);
        assert_eq!(normalize_room_code("ABCDÉF"), None);
        assert_eq!(normalize_room_code(""), None);
    }
}
