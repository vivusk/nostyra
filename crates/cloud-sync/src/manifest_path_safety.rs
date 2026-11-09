//! Whether a path read from a remote manifest (user-editable in a visible cloud
//! folder) is a plain relative path inside the unit that names exactly one file on
//! Windows: no traversal, and no Win32 name aliasing (trailing dot / space that
//! Win32 strips, reserved characters, device names such as `NUL` or `COM1.txt`).

/// Characters Win32 refuses in a file name (besides `\`, `/`, `:` checked apart).
const RESERVED_CHARS: [char; 6] = ['<', '>', '"', '|', '?', '*'];

/// Device names reserved in every folder, with or without an extension.
const DEVICE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Whether a manifest path is a plain relative path inside the unit: non-empty
/// `/`-separated segments, none `.` / `..`, and each segment a name Win32 maps to
/// itself (see [`is_plain_segment`]). Never write outside the unit or onto an alias.
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty() && path.split('/').all(is_plain_segment)
}

/// One path segment: not empty / `.` / `..`; no `\`, `:`, control chars or
/// `<>"|?*`; not ending in `.` or space; not a reserved device name.
fn is_plain_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && !seg.ends_with(['.', ' '])
        && !seg
            .chars()
            .any(|c| c == '\\' || c == ':' || c.is_ascii_control() || RESERVED_CHARS.contains(&c))
        && !is_device_name(seg)
}

/// `CON`, `nul.txt`, `COM1.srm`, `LPT²`, … (stem before the first `.`, trailing
/// spaces ignored as Win32 does, compared case-insensitively).
fn is_device_name(seg: &str) -> bool {
    let stem = seg.split('.').next().unwrap_or(seg).trim_end_matches(' ');
    if DEVICE_NAMES
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name))
    {
        return true;
    }
    let mut chars = stem.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let digit = chars.as_str();
    (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
        && matches!(
            digit,
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_are_safe() {
        for ok in [
            "save.srm",
            "PSP/SAVEDATA/X/DATA.BIN",
            "User/GC/a b.gci",
            "CONSOLE.srm",
            "com10.sav",
            "nullable/x",
            "a.b.c",
        ] {
            assert!(is_safe_relative_path(ok), "{ok}");
        }
    }

    #[test]
    fn traversal_and_separators_are_unsafe() {
        for bad in [
            "", "/abs", "a//b", "../x", "a/../b", "./a", "a\\b", "C:/x", "a/", "a\0b",
        ] {
            assert!(!is_safe_relative_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn windows_aliases_are_unsafe() {
        for bad in [
            "save.srm.",
            "save.srm ",
            "dir./x",
            "dir /x",
            "a<b",
            "a>b",
            "a\"b",
            "a|b",
            "a?b",
            "a*b",
            "a\tb",
            "a\u{1f}b",
            "NUL",
            "nul.txt",
            "dir/Con",
            "aux.tar.gz",
            "PRN .x",
            "COM1",
            "com0.srm",
            "LPT9.sav",
            "lpt¹",
            "COM³.x",
        ] {
            assert!(!is_safe_relative_path(bad), "{bad:?}");
        }
    }
}
