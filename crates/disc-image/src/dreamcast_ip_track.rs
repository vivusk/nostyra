//! Opens the boot data track of a Dreamcast image (the track whose sector 0 is
//! IP.BIN), whatever the container: `.gdi` (first data track at LBA ≥ 45000), Redump
//! `.cue` (first high-density data track), `.chd` (GD-ROM track 3 / MIL-CD data
//! session), `.cdi` (last session's first data track) or a bare track file.

use std::io::Read;
use std::path::Path;

use crate::cdi_image::CdiImage;
use crate::chd_gd_tracks::boot_track;
use crate::chd_track_reader::ChdTrackReader;
use crate::cue_sheet::CueSheet;
use crate::disc_sector_reader::{
    open_cue_track, open_track_file, DiscReader, FileTrackReader, RAW_SECTOR_BYTES,
};
use crate::gdi_sheet::GdiSheet;
use crate::{is_cue_ext, is_gdi_ext, is_m3u_ext, DiscError, CDI_EXT, CHD_EXT};

/// Mode byte of a raw data sector (byte 15): 1 = user data at 16, 2 = at 24.
const MODE_BYTE: usize = 15;

pub(crate) fn open_ip_bin_track(path: &Path) -> Result<Box<dyn DiscReader>, DiscError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_m3u_ext(ext) {
        return Err(DiscError::Format("not a single disc image"));
    }
    if is_gdi_ext(ext) {
        let sheet = GdiSheet::read(path)?;
        let track = sheet
            .high_density_data_track()
            .ok_or(DiscError::Format("GDI has no high-density data track"))?;
        let file = path.parent().unwrap_or(Path::new("")).join(&track.file);
        return Ok(Box::new(open_gdi_track(&file, track.sector_size)?));
    }
    if is_cue_ext(ext) {
        let (file, entry) = CueSheet::read(path)?.boot_data_track(path)?;
        return Ok(Box::new(open_cue_track(&file, &entry)?));
    }
    if ext.eq_ignore_ascii_case(CHD_EXT) {
        return Ok(Box::new(ChdTrackReader::open_track(path, boot_track)?));
    }
    if ext.eq_ignore_ascii_case(CDI_EXT) {
        let image = CdiImage::read(path)?;
        let track = image.boot_track().ok_or(DiscError::NoDataTrack)?;
        return Ok(Box::new(CdiImage::open_track(path, track)?));
    }
    Ok(Box::new(FileTrackReader::open_auto(path)?))
}

/// A GDI data track file from its start: 2048-byte sectors, or raw 2352-byte ones
/// whose mode byte places the user data.
pub(crate) fn open_gdi_track(file: &Path, sector_size: u32) -> Result<FileTrackReader, DiscError> {
    if u64::from(sector_size) != RAW_SECTOR_BYTES {
        return FileTrackReader::open_track(file, u64::from(sector_size), 0, 0, None);
    }
    let mut head = [0u8; MODE_BYTE + 1];
    open_track_file(file)?.read_exact(&mut head)?;
    let data_offset = if head[MODE_BYTE] == 2 { 24 } else { 16 };
    FileTrackReader::open_track(file, RAW_SECTOR_BYTES, data_offset, 0, None)
}
