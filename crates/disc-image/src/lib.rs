//! Reads CD, GD-ROM, UMD and GameCube / Wii disc images without loading them
//! (bounded I/O: a few sectors per disc, one 0x80-byte header for GC/Wii).
//!
//! - [`cue_sheet`]: `.cue` parsing (files, tracks, GD-ROM density areas, missing files).
//! - [`gdi_sheet`]: `.gdi` parsing (Dreamcast GD-ROM track list).
//! - [`cdi_image`]: DiscJuggler `.cdi` session/track table.
//! - [`m3u_playlist`]: `.m3u` multi-disc playlists (≤ 8 discs, the core's limit).
//! - [`disc_sector_reader`]: 2048-byte user data of any sector of the first data
//!   track — raw 2352 MODE1/MODE2, ISO 2048, CSO ([`cso_image`]), or CHD
//!   ([`chd_track_reader`], DVD: [`chd_dvd_reader`]).
//! - [`disc_track_access`]: every track with its absolute LBA (sessions, GD-ROM
//!   high-density area), data tracks opened by number or rcheevos-style pick and
//!   read by absolute LBA (achievement hashing).
//! - [`iso9660_root`]: primary volume descriptor + file lookup by path.
//! - [`playstation_disc_id`]: PS1 recognition (system id + licence), boot serial,
//!   region.
//! - [`dreamcast_disc_id`]: Dreamcast IP.BIN (hardware id, product number, title)
//!   from a `.gdi` / `.cue` / `.chd` / `.cdi` / bare track.
//! - [`disc_serial_rule`]: serial normalisation per disc signature (DAT matching).
//! - [`identity_source`]: which file a disc game's identity hashes.
//! - [`param_sfo`]: `PARAM.SFO` (category, disc id, title, version).
//! - [`pbp_sfo`]: `EBOOT.PBP` — PS1 classic or PSP eboot, its `PARAM.SFO`.
//! - [`psp_disc_id`]: PSP UMD recognition (`PSP GAME`), serial and title.
//! - [`nintendo_disc_header`]: GameCube / Wii disc header (game ID, disc, revision,
//!   system from the magic) out of `.iso/.gcm/.ciso/.wia/.rvz/.wbfs/.gcz`.
//! - [`dol_elf_executable`]: `.dol` / `.elf` checks and the GC/Wii HID4 split.
//! - [`DiscImageProbe`]: the registry's `DiscProbe` (scanner + runner use it).
//!
//! Extension constants are dot-less on purpose: the repo's system-literal gate only
//! allows `".ext"` literals in the registry.

#![deny(unsafe_code)]

pub mod cdi_image;
mod cdi_track_table;
pub mod chd_dvd_reader;
mod chd_gd_tracks;
mod chd_track_list;
pub mod chd_track_reader;
pub mod cso_image;
pub mod cue_sheet;
mod cue_track_mode;
mod disc_error;
mod disc_identity_track;
mod disc_image_probe;
pub mod disc_sector_reader;
pub mod disc_serial_rule;
pub mod disc_track_access;
mod disc_track_lists;
pub mod dol_elf_executable;
pub mod dreamcast_disc_id;
mod dreamcast_ip_track;
pub mod gdi_sheet;
pub mod iso9660_root;
pub mod m3u_playlist;
mod nintendo_disc_containers;
pub mod nintendo_disc_header;
mod nintendo_disc_probe;
pub mod param_sfo;
pub mod pbp_sfo;
pub mod playstation_disc_id;
pub mod psp_disc_id;
mod sheet_text;

pub use cue_sheet::{CueSheet, TrackArea, TrackMode};
pub use disc_error::DiscError;
pub use disc_identity_track::identity_source;
pub use disc_image_probe::DiscImageProbe;
pub use disc_sector_reader::{open_data_track, DiscReader, SECTOR_DATA_BYTES};
pub use disc_serial_rule::{display_serial, serial_lookup_keys};
pub use disc_track_access::{track_list, DiscTrack, TrackInfo, TrackSelector};
pub use dol_elf_executable::{is_executable_ext, read_executable_kind};
pub use dreamcast_disc_id::{read_ip_bin, IpBin};
pub use gdi_sheet::GdiSheet;
pub use m3u_playlist::{parse_playlist, playlist_entries};
pub use nintendo_disc_header::{
    is_nintendo_disc_ext, read_nintendo_disc_header, NintendoDiscHeader, NintendoDiscKind,
};
pub use param_sfo::ParamSfo;
pub use pbp_sfo::{PbpInfo, PbpKind};
pub use playstation_disc_id::{is_playstation, normalize_serial, region, serial, DiscRegion};
pub use psp_disc_id::{read_psp_identity, PspDiscId};

/// Cue sheet extension (dot-less).
pub const CUE_EXT: &str = "cue";
/// Multi-disc playlist extension (dot-less).
pub const M3U_EXT: &str = "m3u";
/// MAME compressed disc image extension (dot-less).
pub const CHD_EXT: &str = "chd";
/// Dreamcast GD-ROM sheet extension (dot-less).
pub const GDI_EXT: &str = "gdi";
/// DiscJuggler image extension (dot-less).
pub const CDI_EXT: &str = "cdi";
/// PSP eboot extension (dot-less).
pub const PBP_EXT: &str = "pbp";
/// Compressed ISO extension (dot-less).
pub const CSO_EXT: &str = "cso";
/// Plain disc image extension (PS1, PSP, GameCube, Wii; dot-less).
pub const ISO_EXT: &str = "iso";
/// GameCube plain disc image extension (dot-less).
pub const GCM_EXT: &str = "gcm";
/// GameCube / Wii compact ISO extension (dot-less; not the PSP `cso`).
pub const CISO_EXT: &str = "ciso";
/// Dolphin's zlib-compressed disc extension (dot-less).
pub const GCZ_EXT: &str = "gcz";
/// Wii ISO Archive extension (dot-less).
pub const WIA_EXT: &str = "wia";
/// Dolphin's RVZ (WIA with zstd and junk removal) extension (dot-less).
pub const RVZ_EXT: &str = "rvz";
/// Wii Backup File System extension (dot-less, Wii only).
pub const WBFS_EXT: &str = "wbfs";
/// GameCube / Wii DOL executable extension (dot-less).
pub const DOL_EXT: &str = "dol";
/// GameCube / Wii ELF executable extension (dot-less).
pub const ELF_EXT: &str = "elf";
/// Most discs one playlist may list (PCSX ReARMed's disk control limit; Flycast
/// takes more, but no Dreamcast game has more than 4 discs).
pub const MAX_PLAYLIST_ENTRIES: usize = 8;

pub fn is_cue_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case(CUE_EXT)
}

pub fn is_m3u_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case(M3U_EXT)
}

pub fn is_gdi_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case(GDI_EXT)
}

/// Whether `ext` is a sheet that references other files (`.cue` / `.gdi` / `.m3u`):
/// such a file is a game entry, and the files it names are not.
pub fn is_sheet_ext(ext: &str) -> bool {
    is_cue_ext(ext) || is_gdi_ext(ext) || is_m3u_ext(ext)
}

/// Synthetic DiscJuggler images.
#[cfg(any(test, feature = "test-images"))]
pub mod test_cdi_image;
/// Synthetic Dreamcast images (`.gdi`, density-area `.cue`, GD CHD, `.cdi`).
#[cfg(any(test, feature = "test-images"))]
pub mod test_gd_images;
/// Synthetic disc images for tests here and in dependent crates (feature
/// `test-images`, dev-dependencies only).
#[cfg(any(test, feature = "test-images"))]
pub mod test_images;
/// Synthetic GameCube / Wii disc headers, containers and executables.
#[cfg(any(test, feature = "test-images"))]
pub mod test_nintendo_images;
/// Synthetic PSP images (`PARAM.SFO`, ISO tree) and containers (PBP, CSO, DVD CHD).
#[cfg(any(test, feature = "test-images"))]
pub mod test_psp_containers;
#[cfg(any(test, feature = "test-images"))]
pub mod test_psp_images;
