//! Defensive scrub of secrets before logs leave the PC in a bug report. Nostyra never
//! logs the RetroAchievements token or password (`RunnerLaunch` / `ServerRequest` Debug
//! redact them), but a core, a future log line or a pasted URL might: every value of a
//! `token` / `password` / `t` key is replaced in the forms `key=value` (URL query,
//! `NOSTYRA_RA_TOKEN=…`), `"key":"value"` (JSON) and `key: "value"` (Rust Debug).
//! Netplay (M20): room codes, seat tokens, TURN credentials, ICE candidate lines
//! (they name IP addresses) and the whole `NOSTYRA_NETPLAY_SESSION` / `ice_servers`
//! JSON values.

mod value_spans;

use value_spans::{
    bracketed_end, line_end, non_empty, quoted_after_colon, quoted_value, skip_spaces, unquoted_end,
};

/// Replacement for a scrubbed value.
pub const REDACTED: &str = "<redacted>";

/// Keys whose values are secrets (matched case-insensitively, `_` counts as a word
/// break so `NOSTYRA_RA_TOKEN` matches `token`). `t` is the rcheevos URL token
/// parameter, so it is matched only in the `t=value` form. `code_verifier` /
/// `client_secret` are the OAuth (cloud saves) secrets; plain `code` is not a key:
/// it would scrub `exit_code=3`. Longer keys sharing a prefix come first.
const SECRET_KEYS: [&str; 11] = [
    "password",
    "token",
    "t",
    "code_verifier",
    "client_secret",
    "room_code",
    "netplay_token",
    "credential",
    "candidate",
    "ice_servers",
    "nostyra_netplay_session",
];

/// How far a value reaches besides the quoted / `key=value` forms.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    Plain,
    /// `candidate:… <ip> <port> …` up to the end of the line.
    Line,
    /// A JSON array / object value (`[…]` / `{…}`), brackets balanced.
    Structured,
}

fn value_kind(key: &str) -> ValueKind {
    match key {
        "candidate" => ValueKind::Line,
        "ice_servers" | "nostyra_netplay_session" => ValueKind::Structured,
        _ => ValueKind::Plain,
    }
}

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
    let kind = value_kind(key);
    match bytes.get(after) {
        Some(b'=') if opens_bracket(bytes, after + 1, kind) => {
            non_empty(after + 1, bracketed_end(bytes, after + 1))
        }
        // `token="…"` is how `tracing` prints a string field.
        Some(b'=') if bytes.get(after + 1) == Some(&b'"') => quoted_value(bytes, after + 2),
        Some(b'=') if kind == ValueKind::Line => non_empty(after + 1, line_end(bytes, after + 1)),
        Some(b'=') => non_empty(after + 1, unquoted_end(bytes, after + 1)),
        _ if *key == "t" => None,
        // `"token": "…"`: skip the key's closing quote, then like the Debug form.
        Some(b'"') => value_after_colon(bytes, after + 1, kind),
        Some(b':') => value_after_colon(bytes, after, kind),
        _ => None,
    }
}

fn opens_bracket(bytes: &[u8], at: usize, kind: ValueKind) -> bool {
    kind == ValueKind::Structured && matches!(bytes.get(at), Some(b'[' | b'{'))
}

/// `: "value"`, or for netplay keys `: [json]` / `:candidate text`.
fn value_after_colon(bytes: &[u8], index: usize, kind: ValueKind) -> Option<(usize, usize)> {
    if kind == ValueKind::Plain {
        return quoted_after_colon(bytes, index);
    }
    let at = skip_spaces(bytes, index);
    if bytes.get(at) != Some(&b':') {
        return None;
    }
    let at = skip_spaces(bytes, at + 1);
    match bytes.get(at) {
        Some(b'"') => quoted_value(bytes, at + 1),
        _ if opens_bracket(bytes, at, kind) => non_empty(at, bracketed_end(bytes, at)),
        Some(_) if kind == ValueKind::Line => non_empty(at, line_end(bytes, at)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "log_secret_scrub_tests.rs"]
mod tests;
