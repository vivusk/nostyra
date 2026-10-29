//! [`DiscImageProbe`]: the registry's [`DiscProbe`] backed by this crate. Decides
//! whether a `.bin` / `.iso` / `.chd` / `.cue` / `.gdi` / `.cdi` / `.m3u` / `.pbp`
//! / `.cso` file (GameCube / Wii: `.gcm` / `.ciso` / `.gcz` / `.wia` / `.rvz` /
//! `.wbfs` / `.dol` / `.elf` too) is a disc of the asked signature (PlayStation,
//! Dreamcast, PSP, GameCube or Wii), with one readable reason when it is not. Sheets
//! are checked for missing files first (the scan reports a broken cue / gdi once,
//! here). GameCube / Wii read one disc header only ([`crate::nintendo_disc_probe`]).

use std::path::Path;

use system_catalog::{DiscProbe, DiscSignature};

use crate::cue_sheet::CueSheet;
use crate::disc_sector_reader::open_data_track;
use crate::dreamcast_disc_id::read_ip_bin;
use crate::gdi_sheet::GdiSheet;
use crate::m3u_playlist::{display_name, playlist_entries};
use crate::nintendo_disc_probe::{check_nintendo, playlist_verdict};
use crate::pbp_sfo::{PbpInfo, PbpKind};
use crate::playstation_disc_id::is_playstation;
use crate::psp_disc_id::read_psp_identity;
use crate::{is_cue_ext, is_gdi_ext, is_m3u_ext, DiscError, CDI_EXT, CSO_EXT, PBP_EXT};

const NOT_PLAYSTATION: &str = "not a PlayStation disc";
const NOT_DREAMCAST: &str = "not a Dreamcast disc";
const NOT_PSP_EBOOT: &str = "not a PSP eboot (PlayStation classic or no game data)";

/// Disc probe used by the library scanner and the runner.
pub struct DiscImageProbe;

impl DiscProbe for DiscImageProbe {
    fn check(&self, signature: DiscSignature, path: &Path) -> Result<(), String> {
        check(signature, path)
    }
}

fn check(signature: DiscSignature, path: &Path) -> Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_m3u_ext(ext) && signature == DiscSignature::Psp {
        return Err("PSP games are not played from playlists".to_owned());
    }
    if is_m3u_ext(ext) {
        let entries = playlist_entries(path).map_err(|e| e.to_string())?;
        if let Some(missing) = entries.iter().find(|entry| !entry.is_file()) {
            return Err(format!(
                "disc '{}' not found — fix the playlist line or rename the file",
                display_name(missing)
            ));
        }
        // Nested playlists are rejected by the parser, so this recursion is depth 2.
        let verdict = entries
            .first()
            .map_or(Ok(()), |first| check(signature, first));
        return playlist_verdict(signature, verdict);
    }
    let missing = if is_cue_ext(ext) {
        CueSheet::read(path)
            .map_err(|e| e.to_string())?
            .missing_files(path)
    } else if is_gdi_ext(ext) {
        GdiSheet::read(path)
            .map_err(|e| e.to_string())?
            .missing_files(path)
    } else {
        Vec::new()
    };
    if let Some(missing) = missing.first() {
        let line = if is_cue_ext(ext) { "FILE" } else { "track" };
        return Err(format!(
            "track file '{missing}' not found — fix the {line} line or rename the file"
        ));
    }
    match signature {
        DiscSignature::PlayStation => check_playstation(path, ext),
        DiscSignature::Dreamcast => check_dreamcast(path, ext),
        DiscSignature::Psp => check_psp(path, ext),
        DiscSignature::GameCube | DiscSignature::Wii => check_nintendo(signature, path, ext),
    }
}

fn check_playstation(path: &Path, ext: &str) -> Result<(), String> {
    if ext.eq_ignore_ascii_case(PBP_EXT) {
        let info = PbpInfo::read(path).map_err(|_| NOT_PLAYSTATION.to_owned())?;
        return if info.is_playstation() && info.serial().is_some() {
            Ok(())
        } else {
            Err(NOT_PLAYSTATION.to_owned())
        };
    }
    if is_gdi_ext(ext) || ext.eq_ignore_ascii_case(CDI_EXT) {
        return Err(NOT_PLAYSTATION.to_owned());
    }
    let verdict = open_data_track(path).and_then(|mut reader| is_playstation(reader.as_mut()));
    match verdict {
        Ok(true) => Ok(()),
        Ok(false) => Err(NOT_PLAYSTATION.to_owned()),
        // Unreadable file / missing referenced file: say so; any structural
        // mismatch (no ISO volume, too short, audio-only cue) is simply "not PS1".
        Err(err @ (DiscError::Io(_) | DiscError::MissingFile(_))) => Err(err.to_string()),
        Err(_) => Err(NOT_PLAYSTATION.to_owned()),
    }
}

/// IP.BIN with the Dreamcast hardware id in the boot track's sector 0. A `.gdi` /
/// `.cdi` structure problem (no high-density track, unknown DiscJuggler version) is
/// reported as is; other mismatches are "not a Dreamcast disc".
fn check_dreamcast(path: &Path, ext: &str) -> Result<(), String> {
    if ext.eq_ignore_ascii_case(PBP_EXT) {
        return Err(NOT_DREAMCAST.to_owned());
    }
    let dc_container = is_gdi_ext(ext) || ext.eq_ignore_ascii_case(CDI_EXT);
    match read_ip_bin(path) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(NOT_DREAMCAST.to_owned()),
        Err(err @ (DiscError::Io(_) | DiscError::MissingFile(_))) => Err(err.to_string()),
        Err(err @ DiscError::Format(_)) if dc_container => Err(err.to_string()),
        Err(_) => Err(NOT_DREAMCAST.to_owned()),
    }
}

/// UMD image (`.iso` / `.cso` / DVD `.chd`) with a PSP game volume, or a PSP eboot
/// with game data. CSO problems (ZSO, version 2) are reported as is.
fn check_psp(path: &Path, ext: &str) -> Result<(), String> {
    if ext.eq_ignore_ascii_case(PBP_EXT) {
        let info = PbpInfo::read(path).map_err(|_| NOT_PSP_EBOOT.to_owned())?;
        return match info.kind == PbpKind::Psp && info.has_game_data {
            true => Ok(()),
            false => Err(NOT_PSP_EBOOT.to_owned()),
        };
    }
    match open_data_track(path).and_then(|mut reader| read_psp_identity(reader.as_mut())) {
        Ok(_) => Ok(()),
        Err(err @ (DiscError::Io(_) | DiscError::MissingFile(_) | DiscError::NotPsp(_))) => {
            Err(err.to_string())
        }
        Err(err @ DiscError::Format(_)) if ext.eq_ignore_ascii_case(CSO_EXT) => {
            Err(err.to_string())
        }
        Err(_) => Err("not a PSP UMD image".to_owned()),
    }
}

#[cfg(test)]
#[path = "disc_image_probe_tests.rs"]
mod tests;
