//! Where a secret value ends, per form: quoted (`"…"`), bare (`key=value`), the
//! rest of a line (ICE `candidate:` lines) or a bracketed JSON value (`[…]` / `{…}`,
//! the netplay session env and ICE server lists).

/// `: "value"` starting at `index` (spaces allowed around the colon).
pub(super) fn quoted_after_colon(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
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
pub(super) fn quoted_value(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let end = bytes[start..]
        .iter()
        .position(|b| *b == b'"' || *b == b'\n')
        .map_or(bytes.len(), |offset| start + offset);
    non_empty(start, end)
}

/// End of a `key=value` value: the next separator of URLs, logs or shells.
pub(super) fn unquoted_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|b| b.is_ascii_whitespace() || b"&\"',;)}]".contains(b))
        .map_or(bytes.len(), |offset| start + offset)
}

pub(super) fn skip_spaces(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index) == Some(&b' ') {
        index += 1;
    }
    index
}

pub(super) fn non_empty(start: usize, end: usize) -> Option<(usize, usize)> {
    (end > start).then_some((start, end))
}

/// Rest of the line from `start` (stops before a quote too: JSON string values).
pub(super) fn line_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|b| matches!(b, b'\n' | b'\r' | b'"'))
        .map_or(bytes.len(), |offset| start + offset)
}

/// End (exclusive) of the `[…]` / `{…}` value opening at `start`, skipping brackets
/// inside JSON strings; an unbalanced value runs to the end of the line.
pub(super) fn bracketed_end(bytes: &[u8], start: usize) -> usize {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (offset, &b) in bytes[start..].iter().enumerate() {
        if in_string {
            match b {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => depth += 1,
            b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return start + offset + 1;
                }
            }
            b'\n' => return start + offset,
            _ => {}
        }
    }
    bytes.len()
}
