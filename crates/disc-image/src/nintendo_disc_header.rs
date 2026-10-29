//! GameCube / Wii disc header (the first 0x80 bytes of the disc): 6-char game ID
//! @0x00, disc number @0x06, revision @0x07, Wii magic `5D1C9EA3` (BE) @0x18,
//! GameCube magic `C2339F3D` (BE) @0x1C. The magic decides the system, never the
//! extension; the extension only picks the container that holds the header
//! ([`crate::nintendo_disc_containers`]: plain ISO/GCM, CISO, WIA/RVZ, WBFS, GCZ).
//! Wii headers are unencrypted. Bounded I/O: ≤ [`MAX_HEADER_READ_BYTES`] and at most
//! two seeks per image, whatever its size (discs are 1.4–8 GB).

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use crate::nintendo_disc_containers::read_container_head;
use crate::DiscError;

/// Header bytes read from every container (WIA/RVZ store no more than this).
pub const DISC_HEADER_BYTES: usize = 0x80;
/// Most bytes one header read may pull from a file (container metadata included).
pub const MAX_HEADER_READ_BYTES: u64 = 64 * 1024;

const WII_MAGIC: u32 = 0x5D1C_9EA3;
const GAMECUBE_MAGIC: u32 = 0xC233_9F3D;

/// Which console a disc header (or a `.dol` / `.elf`) belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NintendoDiscKind {
    GameCube,
    Wii,
}

/// The identity fields of a GameCube / Wii disc header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NintendoDiscHeader {
    /// `GALE01`: system char, 2-char game, region char, 2-char maker (A–Z / 0–9).
    pub game_id: [u8; 6],
    /// 0 = disc 1 (multi-disc games: 1 = disc 2).
    pub disc: u8,
    /// 0 = first release, 1 = Rev 1, …
    pub revision: u8,
    pub kind: NintendoDiscKind,
}

impl NintendoDiscHeader {
    /// The game ID as text (`GALE01`); always 6 ASCII alphanumerics.
    pub fn game_id_text(&self) -> &str {
        // Validated in `parse_disc_header`: never fails.
        std::str::from_utf8(&self.game_id).unwrap_or_default()
    }
}

/// Container extensions whose disc header this module reads (dot-less, any case).
pub fn is_nintendo_disc_ext(ext: &str) -> bool {
    use crate::{CISO_EXT, GCM_EXT, GCZ_EXT, ISO_EXT, RVZ_EXT, WBFS_EXT, WIA_EXT};
    [
        ISO_EXT, GCM_EXT, CISO_EXT, GCZ_EXT, WIA_EXT, RVZ_EXT, WBFS_EXT,
    ]
    .iter()
    .any(|known| ext.eq_ignore_ascii_case(known))
}

/// Header of the GameCube / Wii disc image at `path`; `None` when the file holds
/// no GC/Wii magic (a PS2 / PSP / PC `.iso`, another container, a short file).
pub fn read_nintendo_disc_header(path: &Path) -> Result<Option<NintendoDiscHeader>, DiscError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !is_nintendo_disc_ext(ext) {
        return Ok(None);
    }
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    read_nintendo_disc_header_from(&mut file, ext, file_len)
}

/// [`read_nintendo_disc_header`] over any reader (`file_len` = its total length);
/// the container is chosen by `ext` (a PSP `.cso` shares the `CISO` magic with a
/// GC/Wii `.ciso`). [`DiscError::Format`] = a recognised container or header that
/// is damaged.
pub fn read_nintendo_disc_header_from<R: Read + Seek>(
    reader: &mut R,
    ext: &str,
    file_len: u64,
) -> Result<Option<NintendoDiscHeader>, DiscError> {
    let Some(head) = read_container_head(reader, ext, file_len)? else {
        return Ok(None);
    };
    let header = parse_disc_header(&head)?;
    // WBFS holds Wii discs only; a GameCube header in one is not a game Dolphin
    // would boot from it.
    let wbfs = ext.eq_ignore_ascii_case(crate::WBFS_EXT);
    Ok(header.filter(|h| !(wbfs && h.kind == NintendoDiscKind::GameCube)))
}

/// Kind + identity of raw header bytes; `None` without a GC/Wii magic.
pub fn parse_disc_header(
    head: &[u8; DISC_HEADER_BYTES],
) -> Result<Option<NintendoDiscHeader>, DiscError> {
    let magic_at =
        |at: usize| u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    let kind = if magic_at(0x18) == WII_MAGIC {
        NintendoDiscKind::Wii
    } else if magic_at(0x1C) == GAMECUBE_MAGIC {
        NintendoDiscKind::GameCube
    } else {
        return Ok(None);
    };
    let mut game_id = [0u8; 6];
    game_id.copy_from_slice(&head[..6]);
    let valid = game_id
        .iter()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if !valid {
        return Err(DiscError::Format("damaged disc header"));
    }
    Ok(Some(NintendoDiscHeader {
        game_id,
        disc: head[6],
        revision: head[7],
        kind,
    }))
}

#[cfg(test)]
#[path = "nintendo_disc_header_tests.rs"]
mod tests;
