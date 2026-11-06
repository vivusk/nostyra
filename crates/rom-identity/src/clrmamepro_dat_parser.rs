//! Minimal clrmamepro DAT parser for the libretro-database `metadat/` formats:
//!
//! ```text
//! game (                                   game (                       (metadat/genre)
//!     name "Super Mario World (USA)"           comment "Super Mario World (USA)"
//!     region "USA"                             genre "Platform"
//!     rom ( name "…sfc" size 524288            rom ( crc B19ED489 )
//!           crc B19ED489 sha1 6B47BB75… )   )
//! )
//! ```
//! [`parse_clrmamepro_games`] keeps every scalar `key value` of a game block (so the
//! attribute DATs need no special parser); [`parse_clrmamepro_dat`] is the No-Intro
//! view used for identification. Nested blocks other than `rom` and the `clrmamepro`
//! header block are skipped.

use std::collections::HashMap;

pub use crate::clrmamepro_game_blocks::parse_clrmamepro_games;

/// One ROM of a DAT game. Attribute DATs carry only the CRC (Nintendo) or only a
/// serial + name (PlayStation metadat: no CRC at all).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRom {
    /// ROM file name (`"Game (USA).nes"`), when present.
    pub name: Option<String>,
    pub size: Option<u64>,
    /// Upper-case hex, when present.
    pub crc32: Option<String>,
    /// Upper-case hex, when present.
    pub sha1: Option<String>,
    /// ROM-level `serial` (PSP `releaseyear` metadat keys games only here).
    pub serial: Option<String>,
}

/// One `game ( … )` block: scalar keys in file order plus its ROMs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatGame {
    /// Scalar `key value` pairs (`name`, `comment`, `region`, `genre`, `users`…).
    pub attrs: Vec<(String, String)>,
    pub roms: Vec<DatRom>,
}

impl DatGame {
    /// First value of `key`, if present.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// One ROM of a No-Intro DAT game (a multi-ROM game yields several entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatEntry {
    /// Full No-Intro game name, tags included.
    pub name: String,
    pub region: Option<String>,
    pub size: u64,
    /// Upper-case hex.
    pub crc32: String,
    /// Upper-case hex; a few DAT entries have none.
    pub sha1: Option<String>,
    /// Game-level `serial` (Redump disc DATs: `SLUS-01234`); `None` for No-Intro.
    pub serial: Option<String>,
    /// CRC the metadat attribute files key this game by: the headered `.nes` ROM's
    /// CRC for NES (also for its headerless `.unh` entry), the entry's own CRC
    /// otherwise.
    pub metadata_crc32: String,
}

/// ROM file extension of headered NES dumps; metadat files key NES games by its CRC.
const HEADERED_NES_EXTENSION: &str = ".nes";

/// Parses No-Intro DAT text into ROM entries: every game needs a `name`, every ROM a
/// `size` and `crc`. Errors carry a short description.
///
/// The libretro NES DAT lists the headered `.nes` ROM and the headerless `.unh` ROM
/// of a game in two separate `game` blocks with the same name; after parsing, each
/// headerless entry gets its `.nes` twin's CRC as `metadata_crc32` (own CRC when no
/// single twin is identifiable, e.g. every SNES game).
pub fn parse_clrmamepro_dat(text: &str) -> Result<Vec<DatEntry>, String> {
    let mut entries = Vec::new();
    let mut headered: HashMap<String, Vec<(u64, String)>> = HashMap::new();
    let mut is_headered = Vec::new();
    for game in parse_clrmamepro_games(text)? {
        let name = game.attr("name").ok_or("game without name")?.to_string();
        let region = game.attr("region").map(str::to_string);
        let serial = game.attr("serial").map(str::to_string);
        for rom in game.roms {
            let size = rom.size.ok_or("rom without size or crc")?;
            let crc32 = rom.crc32.ok_or("rom without size or crc")?;
            let nes = rom
                .name
                .as_deref()
                .is_some_and(|file| file.to_ascii_lowercase().ends_with(HEADERED_NES_EXTENSION));
            if nes {
                headered
                    .entry(name.clone())
                    .or_default()
                    .push((size, crc32.clone()));
            }
            is_headered.push(nes);
            entries.push(DatEntry {
                name: name.clone(),
                region: region.clone(),
                size,
                metadata_crc32: crc32.clone(),
                crc32,
                sha1: rom.sha1,
                serial: serial.clone(),
            });
        }
    }
    // A headerless entry takes the CRC of its headered twin; names repeat in the DAT
    // (different dumps), so pair by size and keep the own CRC when not unambiguous.
    for (entry, nes) in entries.iter_mut().zip(is_headered) {
        if nes {
            continue;
        }
        if let Some(crc) = headered
            .get(&entry.name)
            .and_then(|twins| headerless_twin(twins, entry.size))
        {
            entry.metadata_crc32.clone_from(crc);
        }
    }
    Ok(entries)
}

/// Size of the iNES header, and of the optional trainer a headerless dump also drops.
const INES_HEADER_SIZE: u64 = 16;
const INES_TRAINER_SIZE: u64 = 512;

/// CRC of the single headered ROM that `size` (a headerless dump) derives from; the
/// only twin when the name has one, else the one whose size fits.
fn headerless_twin(twins: &[(u64, String)], size: u64) -> Option<&String> {
    if let [(_, crc)] = twins {
        return Some(crc);
    }
    let mut fits = twins.iter().filter(|(nes_size, _)| {
        *nes_size == size + INES_HEADER_SIZE
            || *nes_size == size + INES_HEADER_SIZE + INES_TRAINER_SIZE
    });
    match (fits.next(), fits.next()) {
        (Some((_, crc)), None) => Some(crc),
        _ => None,
    }
}
