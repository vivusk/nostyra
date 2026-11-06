//! What a library row shows from a DAT match: title, DAT name, region and the serial
//! to store. One rule set for the desktop scanner and the web edition.
//!
//! - Title: matched by hash → the DAT title; else the content's own title (PSP eboot,
//!   the ROM inside a ZIP); else the file name ("Game (U) [!]" → "Game"). A serial
//!   match alone is "probable" (translations and hacks share their original's serial),
//!   so it never renames the game.
//! - Serial: discs keep their boot serial, else the matched entry's; cartridges store
//!   their header serial (DS game code) as is.

use crate::dat_index::{display_region, display_title};
use crate::game_identity::{DatMatch, GameIdentity};

/// Row facts of one identified game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatFacts {
    pub title: String,
    /// `games.dat_name`: `None` = not in the DAT ("needs review").
    pub dat_name: Option<String>,
    pub region: Option<String>,
    pub serial: Option<String>,
}

/// Facts for `identity` given its DAT `matched` entries; `file_stem` = the file name
/// without extension, for the fallback title.
pub fn dat_facts(identity: &GameIdentity, matched: DatMatch<'_>, file_stem: &str) -> DatFacts {
    let title = matched
        .by_hash
        .map(|entry| display_title(&entry.name))
        .or_else(|| identity.own_title.clone())
        .unwrap_or_else(|| display_title(file_stem));
    let serial = if identity.disc {
        identity
            .serial
            .clone()
            .or_else(|| matched.matched.and_then(|entry| entry.serial.clone()))
    } else {
        identity.serial.clone()
    };
    DatFacts {
        title,
        dat_name: matched.matched.map(|entry| entry.name.clone()),
        region: matched.matched.and_then(display_region),
        serial,
    }
}
