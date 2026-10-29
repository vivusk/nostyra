//! Serial normalisation per disc signature, for display and DAT matching.
//!
//! - PlayStation: `SLUS_012.34` → `SLUS-01234` (both the disc and Redump use it).
//! - Dreamcast: IP.BIN product numbers and Redump serials differ in form (`MK-51035`
//!   on the disc, `51035` in the US DAT entry; `T7001D  50` vs `T-7001D-50`; comma
//!   lists; `-0`/`-1` disc suffixes). Display = trimmed, upper-cased, blanks → `-`;
//!   lookup keys = alphanumerics only with a leading `MK` dropped, plus the key
//!   without a one-digit disc suffix.
//! - PSP: `UMD_DATA.BIN` has `ULUS-10041`, `PARAM.SFO` `ULUS10041`; Redump lists
//!   `ULUS-10041` (sometimes `ULES-01513/E`). Trimmed, upper-cased, a dash after 4
//!   letters + 5 digits; odd forms (`HONEY-002`) kept as is.
//! - GameCube / Wii: the disc header has the 6-char game ID (`GALE01`), Redump lists
//!   `DL-DOL-GALE-USA` / `RVL-RJ2E-USA` (comma lists, `-0`/`-1` suffixes). Display =
//!   trimmed, upper-cased; lookup key = the 4-char game code (`GALE`) on both sides
//!   (the 2-char maker is not in Redump serials); other forms match exactly only.

use system_catalog::DiscSignature;

use crate::playstation_disc_id::normalize_serial;

/// The serial as shown to the user; `None` when `raw` holds no serial.
pub fn display_serial(signature: DiscSignature, raw: &str) -> Option<String> {
    match signature {
        DiscSignature::PlayStation => normalize_serial(raw),
        DiscSignature::Dreamcast => dreamcast_display(raw),
        DiscSignature::Psp => psp_normalize_serial(raw),
        DiscSignature::GameCube | DiscSignature::Wii => {
            let upper = raw.trim().to_ascii_uppercase();
            (!upper.is_empty()).then_some(upper)
        }
    }
}

/// Every key `raw` matches under (a DAT entry may list several serials, comma
/// separated). The disc side and the DAT side use the same function.
pub fn serial_lookup_keys(signature: DiscSignature, raw: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for part in raw.split(',') {
        match signature {
            DiscSignature::PlayStation => keys.extend(normalize_serial(part)),
            DiscSignature::Dreamcast => keys.extend(dreamcast_keys(part)),
            DiscSignature::Psp => keys.extend(psp_key(part)),
            DiscSignature::GameCube | DiscSignature::Wii => keys.extend(nintendo_game_code(part)),
        }
    }
    keys.dedup();
    keys
}

/// PSP serial for display and lookup: `ulus10041 ` → `ULUS-10041`; `None` when blank.
pub fn psp_normalize_serial(raw: &str) -> Option<String> {
    let upper = raw.trim().to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let bare = bytes.len() == 9
        && bytes[..4].iter().all(u8::is_ascii_alphabetic)
        && bytes[4..].iter().all(u8::is_ascii_digit);
    match upper.as_str() {
        "" => None,
        _ if bare => Some(format!("{}-{}", &upper[..4], &upper[4..])),
        _ => Some(upper),
    }
}

/// Lookup key of one DAT/disc serial: the part before a `/` suffix, normalised.
pub fn psp_key(raw: &str) -> Option<String> {
    psp_normalize_serial(raw.split('/').next().unwrap_or(raw))
}

/// Redump serial prefixes of GameCube (`DL-DOL-`) and Wii (`RVL-`) discs.
const NINTENDO_SERIAL_PREFIXES: [&str; 2] = ["DL-DOL-", "RVL-"];

/// The 4-char game code of a 6-char game ID (`GALE01`) or a Redump serial
/// (`DL-DOL-GALE-USA`, `RVL-RJ2E-USA-1`); `None` for other forms.
fn nintendo_game_code(raw: &str) -> Option<String> {
    let upper = raw.trim().to_ascii_uppercase();
    let is_code = |s: &str| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphanumeric());
    if upper.len() == 6 && upper.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Some(upper[..4].to_owned());
    }
    let rest = NINTENDO_SERIAL_PREFIXES
        .iter()
        .find_map(|prefix| upper.strip_prefix(prefix))?;
    let code = rest.split('-').next().unwrap_or("");
    is_code(code).then(|| code.to_owned())
}

fn dreamcast_display(raw: &str) -> Option<String> {
    let words: Vec<String> = raw
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_uppercase)
        .collect();
    (!words.is_empty()).then(|| words.join("-"))
}

fn dreamcast_keys(raw: &str) -> Vec<String> {
    let Some(display) = dreamcast_display(raw) else {
        return Vec::new();
    };
    let mut keys = vec![dreamcast_key(&display)];
    if let Some((base, suffix)) = display.rsplit_once('-') {
        if suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_digit()) {
            keys.push(dreamcast_key(base));
        }
    }
    keys.retain(|k| !k.is_empty());
    keys.dedup();
    keys
}

/// Alphanumerics only, a leading `MK` (Sega's own titles) dropped.
fn dreamcast_key(display: &str) -> String {
    let key: String = display
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    match key.strip_prefix("MK") {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest.to_owned(),
        _ => key,
    }
}

#[cfg(test)]
#[path = "disc_serial_rule_tests.rs"]
mod tests;
