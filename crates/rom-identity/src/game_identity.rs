//! Everything a scan learns about one game file before any database: its hashes
//! (plus the NES headerless twin), its header / boot serial, a title the image gives
//! itself, and which DAT entry it matches. The desktop scanner (`game-library`) and
//! the web edition both build their library rows from this, so the same file gets
//! the same sha1, DAT name and title on both.

use std::path::Path;

use system_catalog::SystemSpec;

use crate::dat_index::{display_title, DatIndex};
use crate::disc_game_identity::read_disc_identity;
use crate::nds_game_code::cartridge_serial;
use crate::rom_archive::{is_rom_archive, read_rom_archive};
use crate::rom_file_hasher::{hash_headerless, hash_rom_bytes, hash_rom_file, RomHashes};
use crate::serial_match_keys::serial_rule;
use crate::DatEntry;

/// Identity of one game file (cartridge, single-ROM ZIP or disc entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameIdentity {
    /// What is stored (`games.crc32/sha1`) and keys the save folder.
    pub hashes: RomHashes,
    /// NES: hashes without the iNES header (the DAT's `.unh` entries).
    pub headerless: Option<RomHashes>,
    /// Disc boot serial or cartridge header serial (DS game code).
    pub serial: Option<String>,
    /// Title the content names itself by when no DAT entry matches by hash (PSP
    /// eboot, the ROM inside a ZIP).
    pub own_title: Option<String>,
    /// A disc game (serial matched under the system's disc rule).
    pub disc: bool,
}

/// Why a file has no identity.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    /// The ROM inside the ZIP belongs to another system than the one given.
    #[error("ROM in ZIP does not match the system")]
    ZipSystemMismatch,
    #[error("{0}")]
    Read(String),
}

/// Reads and hashes the game at `path` as `system` (see the module doc).
pub fn game_identity(path: &Path, system: &SystemSpec) -> Result<GameIdentity, IdentityError> {
    let read = |err: &dyn std::fmt::Display| IdentityError::Read(err.to_string());
    if is_rom_archive(path) {
        let rom = read_rom_archive(path).map_err(|err| read(&err))?;
        if rom.system != system {
            return Err(IdentityError::ZipSystemMismatch);
        }
        let stem = Path::new(&rom.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&rom.name);
        return Ok(GameIdentity {
            hashes: hash_rom_bytes(&rom.bytes, rom.system),
            headerless: hash_headerless(&rom.bytes, rom.system),
            serial: None,
            own_title: Some(display_title(stem)),
            disc: false,
        });
    }
    if let Some(signature) = serial_rule(system) {
        let identity = read_disc_identity(path, signature).map_err(|err| read(&err))?;
        return Ok(GameIdentity {
            hashes: identity.hashes,
            headerless: None,
            serial: identity.serial,
            own_title: identity.title,
            disc: true,
        });
    }
    let (hashes, bytes) = hash_rom_file(path, system).map_err(|err| read(&err))?;
    Ok(GameIdentity {
        hashes,
        headerless: hash_headerless(&bytes, system),
        serial: cartridge_serial(path, system),
        own_title: None,
        disc: false,
    })
}

/// DAT entries a game matches: `by_hash` (its hashes, or the NES headerless twin) —
/// the DAT title is used — and `matched` = `by_hash`, else by serial (size
/// tie-break; "probable": DAT name and region, the title stays the file's own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatMatch<'a> {
    pub by_hash: Option<&'a DatEntry>,
    pub matched: Option<&'a DatEntry>,
}

/// Matches `identity` against `dat` (see [`DatMatch`]).
pub fn match_dat<'a>(dat: &'a DatIndex, identity: &GameIdentity) -> DatMatch<'a> {
    let by_hash = dat.lookup(&identity.hashes).or_else(|| {
        identity
            .headerless
            .as_ref()
            .and_then(|bare| dat.lookup(bare))
    });
    let matched = by_hash.or_else(|| {
        let size = (identity.hashes.size > 0).then_some(identity.hashes.size);
        identity
            .serial
            .as_deref()
            .and_then(|serial| dat.lookup_serial(serial, size))
    });
    DatMatch { by_hash, matched }
}
