//! PSP UMD identification from its ISO9660 volume (any container: `.iso`, `.cso`,
//! DVD `.chd`): system id `PSP GAME` or a `/PSP_GAME` directory; serial from
//! `UMD_DATA.BIN` (`NSTY-00001|…`, first `|` field), else `PARAM.SFO` `DISC_ID`;
//! title and version from `PSP_GAME/PARAM.SFO`. UMD Video / Audio discs (no
//! `PSP_GAME`) are refused with a reason. Eboots use [`PspDiscId::from_sfo`].

use crate::disc_sector_reader::DiscReader;
use crate::disc_serial_rule::psp_normalize_serial;
use crate::iso9660_root::{read_extent, PrimaryVolume};
use crate::param_sfo::ParamSfo;
use crate::DiscError;

const PSP_SYSTEM_ID: &str = "PSP GAME";
const MEDIA_SYSTEM_IDS: [&str; 2] = ["UMD VIDEO", "UMD AUDIO"];
const GAME_DIR: &str = "PSP_GAME";
/// `UMD_DATA.BIN` is ~ 64 bytes; `PARAM.SFO` a few hundred.
const MAX_UMD_DATA: usize = 256;
const MAX_SFO: usize = 64 * 1024;

/// What a PSP game says about itself (each field may be missing on odd images).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PspDiscId {
    /// `ULUS-10041` form.
    pub serial: Option<String>,
    pub title: Option<String>,
    pub disc_version: Option<String>,
}

impl PspDiscId {
    /// Identity of an eboot from its `PARAM.SFO` alone.
    pub fn from_sfo(sfo: &ParamSfo) -> Self {
        Self {
            serial: sfo.disc_id.as_deref().and_then(psp_normalize_serial),
            title: sfo.title.clone(),
            disc_version: sfo.disc_version.clone(),
        }
    }
}

/// Identity of the UMD image behind `reader`; [`DiscError::NotPsp`] when the volume
/// is not a PSP game, [`DiscError::Format`] without an ISO9660 volume.
pub fn read_psp_identity(reader: &mut dyn DiscReader) -> Result<PspDiscId, DiscError> {
    let pvd = PrimaryVolume::read(reader)?;
    let system_id = pvd.system_id.trim().trim_matches('"').trim();
    let has_game_dir = pvd.find_path(reader, &[GAME_DIR])?.is_some();
    if !has_game_dir && !system_id.eq_ignore_ascii_case(PSP_SYSTEM_ID) {
        let media = MEDIA_SYSTEM_IDS
            .iter()
            .any(|id| system_id.eq_ignore_ascii_case(id));
        return Err(DiscError::NotPsp(if media {
            "UMD Video/Audio disc"
        } else {
            "not a PSP UMD image"
        }));
    }
    let sfo = match pvd.find_path(reader, &[GAME_DIR, "PARAM.SFO"])? {
        Some(extent) => ParamSfo::parse(&read_extent(reader, extent, MAX_SFO)?).ok(),
        None => None,
    }
    .unwrap_or_default();
    let mut id = PspDiscId::from_sfo(&sfo);
    if let Some(extent) = pvd.find_root_file(reader, "UMD_DATA.BIN")? {
        let text = read_extent(reader, extent, MAX_UMD_DATA)?;
        let text = String::from_utf8_lossy(&text);
        let first = text.split('|').next().unwrap_or("");
        if let Some(serial) = psp_normalize_serial(first.trim_end_matches('\0')) {
            id.serial = Some(serial);
        }
    }
    Ok(id)
}

#[cfg(test)]
#[path = "psp_disc_id_tests.rs"]
mod tests;
