//! Memory card files from other emulators (pure): raw 128 KiB cards (ePSXe `.mcr`,
//! DuckStation `.mcd`, Beetle `.srm`), DexDrive `.gme` (3904-byte header) and
//! Connectix `.vgs` / `.mem` (64-byte header) become the raw card the core keeps;
//! the directory frames tell how many saves a card holds and whether one belongs to
//! a game serial (`BASLUS-00594…`).

/// A raw PlayStation memory card: 16 blocks × 8 KiB.
pub const CARD_BYTES: usize = 128 * 1024;
const GME_HEADER: usize = 3904;
const VGS_HEADER: usize = 64;
/// Directory frames 1..=15 (128 bytes each) of block 0; byte 0 = 0x51 marks the
/// first block of a save.
const FRAME_BYTES: usize = 128;
const SAVE_START: u8 = 0x51;
const NAME_AT: usize = 0x0A;
const NAME_LEN: usize = 20;

/// The raw card inside `bytes`, or why it is not a memory card.
pub fn normalize_card(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let raw = match bytes.len() {
        CARD_BYTES => bytes,
        n if n == CARD_BYTES + GME_HEADER => &bytes[GME_HEADER..],
        n if n == CARD_BYTES + VGS_HEADER => &bytes[VGS_HEADER..],
        n => {
            return Err(format!(
                "Not a PlayStation memory card ({n} bytes; a card is 131072 bytes, \
                 or a .gme / .vgs file around one)."
            ))
        }
    };
    if !raw.starts_with(b"MC") {
        return Err("Not a PlayStation memory card (its \"MC\" header is missing).".into());
    }
    Ok(raw.to_vec())
}

/// File names of the saves on a raw card (one per save, first block only).
pub fn save_names(card: &[u8]) -> Vec<String> {
    (1..16)
        .filter_map(|i| card.get(i * FRAME_BYTES..(i + 1) * FRAME_BYTES))
        .filter(|frame| frame[0] == SAVE_START)
        .map(|frame| {
            let name = &frame[NAME_AT..NAME_AT + NAME_LEN];
            let end = name.iter().position(|b| *b == 0).unwrap_or(NAME_LEN);
            String::from_utf8_lossy(&name[..end]).into_owned()
        })
        .collect()
}

/// Whether a save on the card carries `serial` (`SLUS-00594`; also matched without
/// the dash).
pub fn card_has_serial(card: &[u8], serial: &str) -> bool {
    let bare: String = serial.chars().filter(|c| *c != '-').collect();
    save_names(card)
        .iter()
        .any(|name| name.contains(serial) || name.contains(&bare))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Formatted card with one save of `serial` in frame 1.
    fn card(serial: &str) -> Vec<u8> {
        let mut card = vec![0u8; CARD_BYTES];
        card[..2].copy_from_slice(b"MC");
        let frame = &mut card[FRAME_BYTES..2 * FRAME_BYTES];
        frame[0] = SAVE_START;
        let name = format!("BA{serial}SAVE01");
        frame[NAME_AT..NAME_AT + name.len()].copy_from_slice(name.as_bytes());
        card
    }

    #[test]
    fn raw_gme_and_vgs_cards_normalise_to_the_raw_card() {
        let raw = card("SLUS-00594");
        assert_eq!(normalize_card(&raw), Ok(raw.clone()));
        let mut gme = vec![0xAA; GME_HEADER];
        gme.extend(&raw);
        assert_eq!(normalize_card(&gme), Ok(raw.clone()));
        let mut vgs = vec![0xBB; VGS_HEADER];
        vgs.extend(&raw);
        assert_eq!(normalize_card(&vgs), Ok(raw.clone()));
    }

    #[test]
    fn wrong_size_or_missing_header_is_refused() {
        assert!(normalize_card(&[0u8; 1000])
            .unwrap_err()
            .contains("1000 bytes"));
        assert!(normalize_card(&vec![0u8; CARD_BYTES])
            .unwrap_err()
            .contains("MC"));
    }

    #[test]
    fn saves_and_serials_come_from_the_directory() {
        let raw = card("SLUS-00594");
        assert_eq!(save_names(&raw), ["BASLUS-00594SAVE01"]);
        assert!(card_has_serial(&raw, "SLUS-00594"));
        assert!(!card_has_serial(&raw, "SCUS-94426"));
        let mut empty = vec![0u8; CARD_BYTES];
        empty[..2].copy_from_slice(b"MC");
        assert!(save_names(&empty).is_empty());
    }

    /// Dev-only: the developer's ePSXe card for Emblem Saga (read-only).
    #[test]
    #[ignore = "needs the developer's own ePSXe memory card"]
    fn developer_epsxe_card_holds_emblem_saga_saves() {
        let bytes =
            std::fs::read(r"E:\ePSXe v2.0.5\memcards\games\SLPS_031.77-00.mcr").unwrap_or_default();
        let raw = normalize_card(&bytes).unwrap();
        println!("{:?}", save_names(&raw));
        assert!(card_has_serial(&raw, "SLPS-03177"));
    }
}
