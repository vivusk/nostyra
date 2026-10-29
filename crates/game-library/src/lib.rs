//! ROM library: finds ROMs of every registry system in user folders (classified by
//! content, see `system-catalog`; multi-file disc games grouped into one entry),
//! hashes them (CRC32 + SHA1, header rule per system; discs: first data track and
//! large cartridges: streamed), identifies them with the bundled DAT (discs: also
//! by boot serial; DS cartridges: also by header game code) and writes results to
//! storage-db together with offline metadata (vendored libretro-database
//! metadat). Pure Rust, no UI; the launcher runs scans on a worker thread.

#![deny(unsafe_code)]

mod clrmamepro_dat_parser;
mod dat_index;
mod dat_rematch;
mod dat_serial_index;
mod disc_file_grouping;
mod disc_game_identity;
mod folder_scanner;
mod game_content_sha1;
mod gamecube_wii_identity;
mod library_error;
mod library_scan_discovery;
mod library_scan_job;
mod metadata_dat_index;
mod nds_game_code;
mod rom_file_hasher;
mod scanned_game_builder;
mod serial_match_keys;
mod state_content_tag;
mod system_catalogs;

pub use clrmamepro_dat_parser::{
    parse_clrmamepro_dat, parse_clrmamepro_games, DatEntry, DatGame, DatRom,
};
pub use dat_index::{display_region, display_title, DatIndex};
pub use folder_scanner::{discover_rom_files, DiscoveredFile, DiscoveredFiles, ScanFailure};
pub use game_content_sha1::game_content_sha1;
pub use library_error::LibraryError;
pub use library_scan_job::{scan_library, ScanProgress, ScanSummary};
pub use metadata_dat_index::{MetadataIndex, METADATA_PARTIAL_VERSION, METADATA_SOURCE_VERSION};
pub use rom_file_hasher::{
    hash_bytes, hash_file_streaming, hash_headerless, hash_rom_bytes, hash_rom_file, read_rom_file,
    RomHashes,
};
pub use state_content_tag::state_content_tag;
pub use system_catalogs::{SystemCatalog, SystemCatalogs};
