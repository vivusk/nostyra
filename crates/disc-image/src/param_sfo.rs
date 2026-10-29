//! `PARAM.SFO`, the key/value table of a PSP disc (`PSP_GAME/PARAM.SFO`) or eboot
//! (inside `EBOOT.PBP`): `\0PSF` v1.1 header, 16-byte index entries, key table, data
//! table. Values are UTF-8 strings (fmt `0x0204` null-terminated, `0x0004` not) or
//! u32 (`0x0404`, ignored here).

use crate::DiscError;

/// The fields this crate reads (others are skipped).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParamSfo {
    /// `ME` PS1 classic, `UG` UMD game, `EG`/`MG` PSN/homebrew eboot, …
    pub category: Option<String>,
    /// Serial without its dash (`ULUS10041`, `SLUS00594`).
    pub disc_id: Option<String>,
    pub title: Option<String>,
    /// `1.00`, `1.01`, …
    pub disc_version: Option<String>,
}

const HEADER_BYTES: usize = 0x14;
const ENTRY_BYTES: usize = 16;
const FMT_UTF8_SPECIAL: u16 = 0x0004;
const FMT_UTF8: u16 = 0x0204;

impl ParamSfo {
    /// Parses a whole `PARAM.SFO`; entries pointing outside the table are skipped.
    pub fn parse(sfo: &[u8]) -> Result<Self, DiscError> {
        if sfo.len() < HEADER_BYTES || &sfo[..4] != b"\0PSF" {
            return Err(DiscError::Format("no readable PARAM.SFO"));
        }
        let key_table = le32(sfo, 0x08) as usize;
        let data_table = le32(sfo, 0x0C) as usize;
        let count = le32(sfo, 0x10) as usize;
        let mut out = Self::default();
        for i in 0..count {
            let at = HEADER_BYTES + i * ENTRY_BYTES;
            let Some(entry) = sfo.get(at..at + ENTRY_BYTES) else {
                break;
            };
            let key_off = usize::from(u16::from_le_bytes([entry[0], entry[1]]));
            let fmt = u16::from_le_bytes([entry[2], entry[3]]);
            if fmt != FMT_UTF8 && fmt != FMT_UTF8_SPECIAL {
                continue;
            }
            let len = le32(entry, 4) as usize;
            let start = data_table.saturating_add(le32(entry, 12) as usize);
            let key = c_string(sfo.get(key_table.saturating_add(key_off)..).unwrap_or(&[]));
            let value = sfo.get(start..start.saturating_add(len)).map(c_string);
            let slot = match key.as_str() {
                "CATEGORY" => &mut out.category,
                "DISC_ID" => &mut out.disc_id,
                "TITLE" => &mut out.title,
                "DISC_VERSION" => &mut out.disc_version,
                _ => continue,
            };
            *slot = value.filter(|v| !v.is_empty());
        }
        Ok(out)
    }
}

pub(crate) fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// Text up to the first NUL (lossy UTF-8, trimmed).
fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_psp_images::{sfo, SfoValue};

    #[test]
    fn reads_strings_and_skips_numbers() {
        let table = sfo(&[
            ("BOOTABLE", SfoValue::U32(1)),
            ("CATEGORY", SfoValue::Str("UG")),
            ("DISC_ID", SfoValue::Str("NSTY00001")),
            ("DISC_VERSION", SfoValue::Str("1.00")),
            ("TITLE", SfoValue::Str("Nostyra Test PSP")),
        ]);
        let parsed = ParamSfo::parse(&table).unwrap();
        assert_eq!(parsed.category.as_deref(), Some("UG"));
        assert_eq!(parsed.disc_id.as_deref(), Some("NSTY00001"));
        assert_eq!(parsed.disc_version.as_deref(), Some("1.00"));
        assert_eq!(parsed.title.as_deref(), Some("Nostyra Test PSP"));
    }

    #[test]
    fn broken_tables_error_or_skip() {
        assert!(ParamSfo::parse(b"\0PSF tiny").is_err());
        assert!(ParamSfo::parse(&[0u8; 64]).is_err());
        let mut table = sfo(&[("TITLE", SfoValue::Str("x"))]);
        table[0x10..0x14].copy_from_slice(&1000u32.to_le_bytes()); // count past the end
        assert_eq!(ParamSfo::parse(&table).unwrap().title.as_deref(), Some("x"));
        table[0x14 + 12..0x14 + 16].copy_from_slice(&u32::MAX.to_le_bytes()); // data off
        assert_eq!(ParamSfo::parse(&table).unwrap().title, None);
    }
}
