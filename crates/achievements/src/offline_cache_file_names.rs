//! File and folder names of the offline answer cache (`offline_response_cache`):
//! request form values (user, game hash) turned into names that are safe on every
//! OS and identical whatever spelling or encoding a request used.

/// Decodes `%XX` escapes of a form value (rcheevos encodes everything outside
/// `[A-Za-z0-9-_.~]` that way; no `+` for spaces).
pub(crate) fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Folder name of a user: URL-decoded and lower-cased (the typed name of `login2` and
/// the server's spelling of later requests meet), kept when it only has
/// `[a-z0-9._-]`, else `~` + hex of its UTF-8 bytes (`~` never appears in kept names).
pub(crate) fn user_key(user: &str) -> String {
    let name = url_decode(user).to_lowercase();
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if plain {
        return name;
    }
    let hex: String = name.bytes().map(|b| format!("{b:02x}")).collect();
    format!("~{hex}")
}

/// `<hash>.json` for a hex game hash (lower-cased); anything else is not cached.
pub(crate) fn hash_file_name(hash: &str) -> Option<String> {
    let valid = (1..=64).contains(&hash.len()) && hash.bytes().all(|b| b.is_ascii_hexdigit());
    valid.then(|| format!("{}.json", hash.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_values_decode_and_users_get_safe_folder_names() {
        assert_eq!(url_decode("a%2Db%41%2"), "a-bA%2");
        assert_eq!(
            url_decode("%+1x"),
            "%+1x",
            "only two hex digits form an escape"
        );
        assert_eq!(user_key("Tester"), "tester");
        assert_eq!(user_key("Te%2Ds_t.1"), "te-s_t.1");
        assert_eq!(user_key("a/b"), "~612f62");
        assert_eq!(user_key(".."), "~2e2e");
        assert_eq!(user_key(""), "~");
        assert_eq!(hash_file_name("ABCdef01").as_deref(), Some("abcdef01.json"));
        assert_eq!(hash_file_name("../x"), None);
    }
}
