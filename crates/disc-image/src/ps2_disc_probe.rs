//! PlayStation 2 arm of [`crate::DiscImageProbe`]: the first data track of any
//! container (`.iso`, raw `.bin` / `.cue`, CD or DVD `.chd`, `.cso` / `.zso`) must be
//! a `PLAYSTATION` volume whose `SYSTEM.CNF` has a `BOOT2` line. PlayStation 2 is
//! the last `.bin` candidate (`ALL_SYSTEMS`: … PSX, PS2, …; Dreamcast has no `.bin`,
//! the Atari `.bin` is DAT-gated), so on a `.bin` the reject text names both
//! PlayStation systems and becomes the scan-problem row / Atari DAT-pending hint.
//! Also the header-only identity key rom-identity hashes ([`Ps2IdentityKey`]).

use std::path::Path;

use crate::disc_sector_reader::{open_data_track, DiscReader};
use crate::iso9660_root::PrimaryVolume;
use crate::playstation_disc_id::{boot2_exe, boot2_serial, is_playstation2, system_cnf};
use crate::{is_cso_ext, is_gdi_ext, DiscError, BIN_EXT, CDI_EXT, PBP_EXT};

pub(crate) const NOT_PS2: &str = "not a PlayStation 2 disc";
pub(crate) const NOT_ANY_BIN: &str = "not a PlayStation or PlayStation 2 disc";

/// Checks `path` (extension `ext`) against the PlayStation 2 signature. Unreadable
/// files, missing cue tracks and damaged `.cso` / `.zso` containers keep their own
/// text; every structural mismatch (no ISO volume, audio-only cue, too short) is the
/// plain reject text.
pub(crate) fn check_ps2(path: &Path, ext: &str) -> Result<(), String> {
    // Eboots and Dreamcast containers never hold a PS2 disc.
    if ext.eq_ignore_ascii_case(PBP_EXT) || is_gdi_ext(ext) || ext.eq_ignore_ascii_case(CDI_EXT) {
        return Err(reject_text(ext).to_owned());
    }
    match open_data_track(path).and_then(|mut reader| is_playstation2(reader.as_mut())) {
        Ok(true) => Ok(()),
        Ok(false) => Err(reject_text(ext).to_owned()),
        Err(err @ (DiscError::Io(_) | DiscError::MissingFile(_))) => Err(err.to_string()),
        Err(err @ DiscError::Format(_)) if is_cso_ext(ext) => Err(err.to_string()),
        Err(_) => Err(reject_text(ext).to_owned()),
    }
}

/// What a PlayStation 2 disc is identified by, read from its header only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ps2IdentityKey {
    /// `BOOT2` names a serial-shaped executable (`SLUS_203.12;1` → `SLUS-20312`).
    Serial(String),
    /// No serial in `BOOT2` (homebrew / demo discs booting `BOOT.ELF`, `PS2LOGO`,
    /// `DEMO\…`): the PVD volume id and the upper-cased boot path (`;1` kept).
    VolumeExe { volume_id: String, exe: String },
}

impl Ps2IdentityKey {
    /// The serial, when the disc has one.
    pub fn serial(&self) -> Option<&str> {
        match self {
            Self::Serial(serial) => Some(serial),
            Self::VolumeExe { .. } => None,
        }
    }
}

/// The PS2 identity key of the volume: its `BOOT2` serial, else volume id + boot
/// path; `None` without a `SYSTEM.CNF` / `BOOT2` line (not a PS2 disc).
pub fn ps2_identity_key(reader: &mut dyn DiscReader) -> Result<Option<Ps2IdentityKey>, DiscError> {
    let pvd = PrimaryVolume::read(reader)?;
    let Some(cnf) = system_cnf(reader, &pvd)? else {
        return Ok(None);
    };
    let Some(exe) = boot2_exe(&cnf) else {
        return Ok(None);
    };
    Ok(Some(match boot2_serial(&cnf) {
        Some(serial) => Ps2IdentityKey::Serial(serial),
        None => Ps2IdentityKey::VolumeExe {
            volume_id: pvd.volume_id,
            exe,
        },
    }))
}

fn reject_text(ext: &str) -> &'static str {
    if ext.eq_ignore_ascii_case(BIN_EXT) {
        NOT_ANY_BIN
    } else {
        NOT_PS2
    }
}

#[cfg(test)]
#[path = "ps2_disc_probe_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ps2_disc_id_tests.rs"]
mod id_tests;
