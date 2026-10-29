//! Defensive scrub of secrets before logs leave the PC in a bug report. Nostyra never
//! logs the RetroAchievements token or password (`RunnerLaunch` / `ServerRequest` Debug
//! redact them), but a core, a future log line or a pasted URL might: every value of a
//! `token` / `password` / `t` key is replaced in the forms `key=value` (URL query,
//! `NOSTYRA_RA_TOKEN=…`), `"key":"value"` (JSON) and `key: "value"` (Rust Debug).

/// Replacement for a scrubbed value.
pub const REDACTED: &str = "<redacted>";

/// Keys whose values are secrets (matched case-insensitively, `_` counts as a word
/// break so `NOSTYRA_RA_TOKEN` matches `token`). `t` is the rcheevos URL token
/// parameter, so it is matched only in the `t=value` form.
const SECRET_KEYS: [&str; 3] = ["password", "token", "t"];

/// `text` with every secret value replaced by [`REDACTED`].
pub fn scrub_secrets(text: &str) -> String {
    // ASCII lowercasing keeps byte offsets, and every cut falls next to an ASCII byte,
    // so the offsets are valid char boundaries of `text` too.
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out = String::with_capacity(text.len());
    let (mut index, mut copied) = (0, 0);
    while index < bytes.len() {
        match secret_value_at(bytes, index) {
            Some((start, end)) => {
                out.push_str(&text[copied..start]);
                out.push_str(REDACTED);
                copied = end;
                index = end;
            }
            None => index += 1,
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Byte range of a secret value whose key starts at `index`.
fn secret_value_at(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    if index > 0 && bytes[index - 1].is_ascii_alphanumeric() {
        return None;
    }
    let rest = &bytes[index..];
    let key = SECRET_KEYS
        .iter()
        .find(|key| rest.starts_with(key.as_bytes()))?;
    let after = index + key.len();
    if bytes
        .get(after)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        return None; // longer word ("tokens", "token_type")
    }
    match bytes.get(after) {
        // `token="…"` is how `tracing` prints a string field.
        Some(b'=') if bytes.get(after + 1) == Some(&b'"') => quoted_value(bytes, after + 2),
        Some(b'=') => non_empty(after + 1, unquoted_end(bytes, after + 1)),
        _ if *key == "t" => None,
        // `"token": "…"`: skip the key's closing quote, then like the Debug form.
        Some(b'"') => quoted_after_colon(bytes, after + 1),
        Some(b':') => quoted_after_colon(bytes, after),
        _ => None,
    }
}

/// `: "value"` starting at `index` (spaces allowed around the colon).
fn quoted_after_colon(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut at = skip_spaces(bytes, index);
    if bytes.get(at) != Some(&b':') {
        return None;
    }
    at = skip_spaces(bytes, at + 1);
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    quoted_value(bytes, at + 1)
}

/// A value from `start` (just after its opening quote) to the closing quote.
fn quoted_value(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let end = bytes[start..]
        .iter()
        .position(|b| *b == b'"' || *b == b'\n')
        .map_or(bytes.len(), |offset| start + offset);
    non_empty(start, end)
}

/// End of a `key=value` value: the next separator of URLs, logs or shells.
fn unquoted_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|b| b.is_ascii_whitespace() || b"&\"',;)}]".contains(b))
        .map_or(bytes.len(), |offset| start + offset)
}

fn skip_spaces(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index) == Some(&b' ') {
        index += 1;
    }
    index
}

fn non_empty(start: usize, end: usize) -> Option<(usize, usize)> {
    (end > start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_url_env_json_and_debug_forms() {
        let text = "GET /dorequest.php?r=awardachievement&u=Tester&t=tok123&a=1\n\
                    NOSTYRA_RA_TOKEN=tok123 set\n\
                    {\"Success\":true,\"Token\":\"tok123\",\"User\":\"Tester\"}\n\
                    Credentials { user: \"Tester\", token: \"tok123\" } password=hunter2\n\
                    INFO login token=\"tok123\" ok\n";
        let scrubbed = scrub_secrets(text);
        assert!(
            !scrubbed.contains("tok123") && !scrubbed.contains("hunter2"),
            "{scrubbed}"
        );
        assert!(scrubbed.contains("u=Tester&t=<redacted>&a=1"), "{scrubbed}");
        assert!(
            scrubbed.contains("NOSTYRA_RA_TOKEN=<redacted> set"),
            "{scrubbed}"
        );
        assert!(
            scrubbed.contains("\"Token\":\"<redacted>\",\"User\":\"Tester\""),
            "{scrubbed}"
        );
        assert!(scrubbed.contains("token: \"<redacted>\" }"), "{scrubbed}");
        assert!(scrubbed.contains("token=\"<redacted>\" ok"), "{scrubbed}");
    }

    #[test]
    fn leaves_ordinary_log_lines_alone() {
        let text = "2026-10-29 15:40:02 INFO set=3 at=5 tokens: 4 rom=\"C:\\Users\\Đức\\a.sfc\"\n";
        assert_eq!(scrub_secrets(text), text);
    }
}
