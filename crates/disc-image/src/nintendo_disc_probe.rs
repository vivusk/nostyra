//! GameCube / Wii arms of [`crate::DiscImageProbe`]: a disc container must hold a
//! header whose magic names the asked system; a `.dol` / `.elf` must be a valid
//! PowerPC executable whose HID4 rule names it. Reject texts follow the classifier
//! order (`ALL_SYSTEMS`: … PSP, GameCube, Wii): Wii is the last `.iso` candidate and
//! GameCube the last `.m3u` candidate, so their texts become the scan-problem row
//! text and name every system that was tried.

use std::path::Path;

use system_catalog::DiscSignature;

use crate::dol_elf_executable::{is_executable_ext, read_executable_kind};
use crate::nintendo_disc_header::{read_nintendo_disc_header, NintendoDiscKind};
use crate::{DiscError, ISO_EXT, WBFS_EXT};

pub(crate) const NOT_GAMECUBE: &str = "not a GameCube disc";
pub(crate) const NOT_GAMECUBE_PLAYLIST: &str = "not a PlayStation, Dreamcast or GameCube playlist";
pub(crate) const NOT_ANY_ISO: &str = "not a PlayStation, PSP, GameCube or Wii disc";
pub(crate) const NOT_WII: &str = "not a Wii disc";
pub(crate) const NOT_GAMECUBE_OR_WII: &str = "not a GameCube or Wii disc";

/// Checks `path` (extension `ext`) against the GameCube or Wii signature. Unreadable
/// files and damaged recognised containers keep their own text.
pub(crate) fn check_nintendo(
    signature: DiscSignature,
    path: &Path,
    ext: &str,
) -> Result<(), String> {
    let wanted = match signature {
        DiscSignature::Wii => NintendoDiscKind::Wii,
        _ => NintendoDiscKind::GameCube,
    };
    let found = if is_executable_ext(ext) {
        read_executable_kind(path)
    } else {
        read_nintendo_disc_header(path).map(|header| header.map(|h| h.kind))
    };
    match found {
        Ok(Some(kind)) if kind == wanted => Ok(()),
        Err(err @ (DiscError::Io(_) | DiscError::Format(_))) => Err(err.to_string()),
        _ => Err(reject_text(signature, ext).to_owned()),
    }
}

/// The verdict of a `.m3u` checked through its first disc: a GameCube playlist whose
/// first disc is not a GameCube disc names every playlist system (GameCube is the
/// last `.m3u` candidate); other verdicts pass through.
pub(crate) fn playlist_verdict(
    signature: DiscSignature,
    verdict: Result<(), String>,
) -> Result<(), String> {
    match verdict {
        Err(text) if signature == DiscSignature::GameCube && text == NOT_GAMECUBE => {
            Err(NOT_GAMECUBE_PLAYLIST.to_owned())
        }
        other => other,
    }
}

fn reject_text(signature: DiscSignature, ext: &str) -> &'static str {
    if signature != DiscSignature::Wii {
        NOT_GAMECUBE
    } else if ext.eq_ignore_ascii_case(ISO_EXT) {
        NOT_ANY_ISO
    } else if ext.eq_ignore_ascii_case(WBFS_EXT) {
        NOT_WII
    } else {
        NOT_GAMECUBE_OR_WII
    }
}

#[cfg(test)]
#[path = "nintendo_disc_probe_tests.rs"]
mod tests;
