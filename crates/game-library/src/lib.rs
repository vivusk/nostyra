//! ROM library: finds ROMs of every registry system in user folders (classified by
//! content, see `system-catalog`; multi-file disc games grouped into one entry),
//! hashes them (CRC32 + SHA1, header rule per system; discs: first data track and
//! large cartridges: streamed), identifies them with the bundled DAT (discs: also
//! by boot serial; DS cartridges: also by header game code) and writes results to
//! storage-db together with offline metadata (vendored libretro-database
//! metadat). Pure Rust, no UI; the launcher runs scans on a worker thread.

#![deny(unsafe_code)]

// Identity code lives in `rom-identity` (shared with the web edition); these aliases
// keep the `crate::<module>` paths of the scanner modules unchanged.
use rom_identity::{dat_index, rom_archive, rom_file_hasher, serial_match_keys};

mod dat_pending_resolution;
mod dat_rematch;
mod disc_file_grouping;
mod folder_scanner;
mod library_error;
mod library_scan_discovery;
mod library_scan_job;
mod library_scan_options;
mod library_scan_summary;
mod scanned_game_builder;
mod system_catalogs;

pub use dat_pending_resolution::DatPendingFile;
pub use folder_scanner::{
    discover_rom_files, discover_rom_files_with_options, DiscoveredFile, DiscoveredFiles,
    ScanFailure,
};
pub use library_error::LibraryError;
pub use library_scan_job::{scan_library, scan_library_with_options, ScanProgress, ScanSummary};
pub use library_scan_options::{read_scan_options, set_zip_scanning, ScanOptions};
pub use rom_identity::{
    dat_pending_hint, display_region, display_title, game_content_sha1, hash_bytes,
    hash_file_streaming, hash_headerless, hash_rom_bytes, hash_rom_file, is_rom_archive,
    parse_clrmamepro_dat, parse_clrmamepro_games, read_rom_archive, read_rom_file,
    state_content_tag, ArchivedRom, DatEntry, DatGame, DatIndex, DatLoadError, DatRom,
    MetadataIndex, MetadataRecord, RomHashes, METADATA_PARTIAL_VERSION, METADATA_SOURCE_VERSION,
};
pub use system_catalogs::{SystemCatalog, SystemCatalogs};
