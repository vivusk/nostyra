//! Identity of a disc game without loading it: the hashes of the file its system's
//! rule picks (`disc_image::identity_source`: PlayStation = first data track,
//! Dreamcast = largest data track — what each Redump DAT hashes; `.chd` / `.cdi` /
//! `.pbp` hash the container, so those never match by hash) and the serial
//! (PlayStation: `SYSTEM.CNF` boot file; Dreamcast: IP.BIN product number; PSP:
//! `UMD_DATA.BIN` / `PARAM.SFO`). A PSP `.pbp` also gives its `PARAM.SFO` title
//! (homebrew and PSN eboots are all named `EBOOT`). GameCube / Wii: game ID only,
//! no whole-file hash (`gamecube_wii_identity`). An `.m3u` game is identified by
//! its first disc.

use std::path::{Path, PathBuf};

use disc_image::{
    display_serial, identity_source, is_m3u_ext, open_data_track, playlist_entries, read_ip_bin,
    read_psp_identity, serial, DiscError, PbpInfo, PspDiscId, PBP_EXT,
};
use system_catalog::DiscSignature;

use crate::gamecube_wii_identity::read_gamecube_wii_identity;
use crate::rom_file_hasher::{hash_file_streaming, RomHashes};

/// What a disc game is identified by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscIdentity {
    pub hashes: RomHashes,
    /// Boot serial (`SLUS-01234`), when the disc has one.
    pub serial: Option<String>,
    /// Title the image names itself by (PSP `.pbp` only), for rows without a DAT
    /// hash match.
    pub title: Option<String>,
}

/// Reads the identity of the disc game whose entry file is `entry`, under the rule
/// of the system whose `signature` claimed it (a `.cue` / `.chd` / `.m3u` is shared).
pub(crate) fn read_disc_identity(
    entry: &Path,
    signature: DiscSignature,
) -> Result<DiscIdentity, DiscError> {
    let disc = first_disc(entry)?;
    if matches!(signature, DiscSignature::GameCube | DiscSignature::Wii) {
        return read_gamecube_wii_identity(&disc, signature);
    }
    let is_pbp = extension(&disc).eq_ignore_ascii_case(PBP_EXT);
    let mut title = None;
    let serial = match signature {
        DiscSignature::PlayStation if is_pbp => PbpInfo::read(&disc)?.serial(),
        DiscSignature::Psp if is_pbp => {
            let id = PspDiscId::from_sfo(&PbpInfo::read(&disc)?.sfo);
            title = id.title.or_else(|| folder_name(&disc));
            id.serial
        }
        DiscSignature::PlayStation => {
            open_data_track(&disc).and_then(|mut reader| serial(reader.as_mut()))?
        }
        DiscSignature::Dreamcast => {
            read_ip_bin(&disc)?.and_then(|ip| display_serial(DiscSignature::Dreamcast, &ip.product))
        }
        DiscSignature::Psp => {
            open_data_track(&disc)
                .and_then(|mut reader| read_psp_identity(reader.as_mut()))?
                .serial
        }
        // Returned above: header-only identity, never a sector image read.
        DiscSignature::GameCube | DiscSignature::Wii => None,
    };
    let hashed = identity_source(&disc, signature)?;
    Ok(DiscIdentity {
        hashes: hash_file_streaming(&hashed)?,
        serial,
        title,
    })
}

/// Name of the folder holding `file` (`MyGame/EBOOT.PBP` → `MyGame`).
fn folder_name(file: &Path) -> Option<String> {
    let name = file.parent()?.file_name()?.to_str()?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// `entry`, or the first disc of an `.m3u` playlist.
fn first_disc(entry: &Path) -> Result<PathBuf, DiscError> {
    if !is_m3u_ext(extension(entry)) {
        return Ok(entry.to_path_buf());
    }
    playlist_entries(entry)?
        .into_iter()
        .next()
        .ok_or(DiscError::EmptyPlaylist)
}

fn extension(path: &Path) -> &str {
    path.extension().and_then(|e| e.to_str()).unwrap_or("")
}

#[cfg(test)]
#[path = "disc_game_identity_tests.rs"]
mod tests;
