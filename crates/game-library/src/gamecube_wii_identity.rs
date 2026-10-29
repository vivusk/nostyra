//! Identity of a GameCube / Wii game without reading the disc: discs are 1.4–8 GB
//! in plain and compressed containers, and compressed ones never match a Redump
//! hash anyway. A disc is keyed by its 6-char game ID (one ≤ 64 KiB header read):
//! serial = the ID (`GALE01`, matched to Redump `DL-DOL-GALE-USA` by the 4-char
//! code), content id = SHA1 of the canonical text `"<system id>:<ID6>"`. The disc
//! byte (0x06) and the revision byte (0x07) are left out on purpose, so every disc
//! of a set, its `.m3u` and every revision — and the ISO, RVZ, WBFS … copies of one
//! of them — share one content id and one save folder (memory card, NAND, shader
//! cache). Homebrew `.dol` / `.elf` files are small: whole-file hash, no serial.

use std::path::Path;

use disc_image::{is_executable_ext, read_nintendo_disc_header, DiscError};
use system_catalog::{DiscSignature, GC, WII};

use crate::disc_game_identity::DiscIdentity;
use crate::rom_file_hasher::{hash_bytes, hash_file_streaming};

/// Shown when a file claimed as GameCube / Wii has no disc header any more (it
/// changed between classification and identification).
const NO_HEADER: &str = "not a GameCube or Wii disc";

/// Identity of the GameCube / Wii `disc` (an `.m3u` is already resolved to its
/// first disc) under `signature` (`GameCube` or `Wii`, the system that claimed it).
pub(crate) fn read_gamecube_wii_identity(
    disc: &Path,
    signature: DiscSignature,
) -> Result<DiscIdentity, DiscError> {
    let ext = disc.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_executable_ext(ext) {
        return Ok(DiscIdentity {
            hashes: hash_file_streaming(disc)?,
            serial: None,
            title: None,
        });
    }
    let header = read_nintendo_disc_header(disc)?.ok_or(DiscError::Format(NO_HEADER))?;
    let game_id = header.game_id_text();
    // The file size keeps the DAT size tie-break meaningful for plain images.
    let size = std::fs::metadata(disc)?.len();
    Ok(DiscIdentity {
        hashes: hash_bytes(canonical_id(signature, game_id).as_bytes(), size),
        serial: Some(game_id.to_owned()),
        title: None,
    })
}

/// `"<system id>:<ID6>"` (`gc:GALE01`), the text a GC / Wii content id hashes.
fn canonical_id(signature: DiscSignature, game_id: &str) -> String {
    let system = if signature == DiscSignature::Wii {
        WII.id
    } else {
        GC.id
    };
    format!("{system}:{game_id}")
}
