//! DVD CHD (chdman `createdvd`, PSP UMD images): plain 2048-byte sectors packed into
//! hunks (usually 4096 B), unit size 2048, no CD/GD track metadata (maybe a `DVD `
//! tag). A `.chd` with CD/GD tracks goes to [`ChdTrackReader`] instead.

use content_file_source::ContentFile;
use std::io::BufReader;
use std::path::Path;

use chd::metadata::Metadata;
use chd::Chd;

use crate::chd_gd_tracks::track_table;
use crate::chd_track_reader::ChdTrackReader;
use crate::disc_sector_reader::{DiscReader, SectorData, SECTOR_DATA_BYTES};
use crate::DiscError;

/// Refuse absurd header values before allocating a hunk buffer.
const MAX_HUNK_BYTES: u64 = 1024 * 1024;

pub struct ChdDvdReader {
    chd: Chd<BufReader<ContentFile>>,
    hunk_bytes: u64,
    sectors: u64,
    hunk_buf: Vec<u8>,
    cmp_buf: Vec<u8>,
    cached_hunk: Option<u32>,
}

/// Opens a `.chd`: a DVD image as plain sectors, a CD/GD image's track 1 otherwise.
pub fn open_chd(path: &Path) -> Result<Box<dyn DiscReader>, DiscError> {
    let mut chd = Chd::open(BufReader::new(content_file_source::open(path)?), None)?;
    let metas: Vec<Metadata> = chd.metadata_refs().try_into()?;
    let dvd =
        chd.header().unit_bytes() as usize == SECTOR_DATA_BYTES && track_table(&metas).is_empty();
    if !dvd {
        return Ok(Box::new(ChdTrackReader::open(path)?));
    }
    let hunk_bytes = u64::from(chd.header().hunk_size());
    if hunk_bytes == 0 || hunk_bytes > MAX_HUNK_BYTES || hunk_bytes % SECTOR_DATA_BYTES as u64 != 0
    {
        return Err(DiscError::Format("CHD is not a DVD image (hunk size)"));
    }
    let sectors = chd.header().logical_bytes() / SECTOR_DATA_BYTES as u64;
    let hunk_buf = chd.get_hunksized_buffer();
    Ok(Box::new(ChdDvdReader {
        chd,
        hunk_bytes,
        sectors,
        hunk_buf,
        cmp_buf: Vec::new(),
        cached_hunk: None,
    }))
}

/// Hunk index and in-hunk byte offset of sector `lba` in a DVD CHD of `hunk_bytes`
/// hunks (2048-byte sectors; the byte offset is computed in `u64`, so discs past
/// 4 GiB — dual-layer DVD-9 — address the right hunk). `SectorOutOfRange` when the
/// hunk index does not fit the CHD's `u32` hunk numbers.
pub fn hunk_of(lba: u32, hunk_bytes: u64) -> Result<(u32, usize), DiscError> {
    let byte = u64::from(lba) * SECTOR_DATA_BYTES as u64;
    let hunk = u32::try_from(byte / hunk_bytes).map_err(|_| DiscError::SectorOutOfRange(lba))?;
    Ok((hunk, (byte % hunk_bytes) as usize))
}

impl DiscReader for ChdDvdReader {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError> {
        if u64::from(lba) >= self.sectors {
            return Err(DiscError::SectorOutOfRange(lba));
        }
        let (hunk, at) = hunk_of(lba, self.hunk_bytes)?;
        if self.cached_hunk != Some(hunk) {
            self.cached_hunk = None;
            self.chd
                .hunk(hunk)?
                .read_hunk_in(&mut self.cmp_buf, &mut self.hunk_buf)?;
            self.cached_hunk = Some(hunk);
        }
        let mut data = [0u8; SECTOR_DATA_BYTES];
        let src = self
            .hunk_buf
            .get(at..at + SECTOR_DATA_BYTES)
            .ok_or(DiscError::SectorOutOfRange(lba))?;
        data.copy_from_slice(src);
        Ok(data)
    }

    fn sector_count(&self) -> u64 {
        self.sectors
    }
}
