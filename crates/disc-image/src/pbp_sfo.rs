//! `EBOOT.PBP`: the `\0PBP` header points at a plain `PARAM.SFO` (readable even when
//! the game data is encrypted) and at the `DATA.PSAR` blob. PS1 classics have
//! `CATEGORY` "ME" and a `PSISOIMG` / `PSTITLEIMG` PSAR; everything else (PSN
//! `NPUMDIMG`, homebrew with an empty PSAR) is a PSP eboot. `DISC_ID` is the serial
//! without its dash (`SLUS00594`, `ULUS10041`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::param_sfo::{le32, ParamSfo};
use crate::playstation_disc_id::normalize_serial;
use crate::DiscError;

/// `PARAM.SFO` is a few hundred bytes; cap the read.
const MAX_SFO_BYTES: u32 = 64 * 1024;
const PS1_CATEGORY: &str = "ME";
/// PSAR magics of PS1 classics (single disc, multi-disc).
const PS1_PSAR_MAGICS: [&[u8]; 2] = [b"PSISOIMG", b"PSTITLEIMG"];
const DATA_PSP_OFFSET_AT: usize = 0x20;
const PSAR_OFFSET_AT: usize = 0x24;

/// Which console an eboot is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbpKind {
    Ps1,
    Psp,
}

/// The `PARAM.SFO` fields of a `.pbp` and its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbpInfo {
    pub sfo: ParamSfo,
    pub kind: PbpKind,
    /// `DATA.PSP` (the executable) is not empty.
    pub has_game_data: bool,
}

impl PbpInfo {
    /// Reads the header, `PARAM.SFO` and PSAR magic of a `.pbp` file.
    pub fn read(path: &Path) -> Result<Self, DiscError> {
        let mut file = File::open(path)?;
        let mut header = [0u8; 0x28];
        file.read_exact(&mut header)
            .map_err(|_| DiscError::Format("file too short for a PBP header"))?;
        if &header[..4] != b"\0PBP" {
            return Err(DiscError::Format("not a PBP file"));
        }
        let sfo_start = le32(&header, 0x08);
        let sfo_end = le32(&header, 0x0C);
        let len = sfo_end
            .checked_sub(sfo_start)
            .filter(|len| (20..=MAX_SFO_BYTES).contains(len))
            .ok_or(DiscError::Format("PBP has no readable PARAM.SFO"))?;
        let mut sfo = vec![0u8; len as usize];
        file.seek(SeekFrom::Start(u64::from(sfo_start)))?;
        file.read_exact(&mut sfo)?;
        let sfo = ParamSfo::parse(&sfo)
            .map_err(|_| DiscError::Format("PBP has no readable PARAM.SFO"))?;
        let psar_at = le32(&header, PSAR_OFFSET_AT);
        let has_game_data = psar_at > le32(&header, DATA_PSP_OFFSET_AT);
        let psar = psar_magic(&mut file, psar_at)?;
        let ps1 = sfo.category.as_deref() == Some(PS1_CATEGORY)
            || PS1_PSAR_MAGICS.iter().any(|m| psar.starts_with(m));
        let kind = if ps1 { PbpKind::Ps1 } else { PbpKind::Psp };
        Ok(Self {
            sfo,
            kind,
            has_game_data,
        })
    }

    /// Whether this is a PlayStation (PS1) classic the PS1 core can run: `CATEGORY`
    /// "ME". A PS1-kind PSAR with another category is claimed by no system.
    pub fn is_playstation(&self) -> bool {
        self.sfo.category.as_deref() == Some(PS1_CATEGORY)
    }

    /// `DISC_ID` in PS1 `SLUS-00594` form.
    pub fn serial(&self) -> Option<String> {
        self.sfo.disc_id.as_deref().and_then(normalize_serial)
    }
}

/// First bytes of `DATA.PSAR` (empty when the PSAR is empty or past the end).
fn psar_magic(file: &mut File, offset: u32) -> Result<Vec<u8>, DiscError> {
    let mut magic = Vec::with_capacity(10);
    if u64::from(offset) < file.metadata()?.len() {
        file.seek(SeekFrom::Start(u64::from(offset)))?;
        file.by_ref().take(10).read_to_end(&mut magic)?;
    }
    Ok(magic)
}

#[cfg(test)]
#[path = "pbp_sfo_tests.rs"]
mod tests;
