//! Game identity without a database, shared by the desktop scanner (`game-library`),
//! the runner and the web edition (wasm32): header-aware CRC32 + SHA1 of cartridges
//! (`rom_file_hasher`), single-ROM ZIPs (`rom_archive`), disc identity — first data
//! track hash + boot serial (`disc_game_identity`), DS / GameCube / Wii codes, the
//! sha1 that keys a game's save folder (`game_content_sha1`), the save-state content
//! tag, No-Intro DAT parsing and matching (`dat_index`), offline metadata from the
//! metadat attribute DATs (`metadata_dat_index`) and multi-file disc grouping
//! (`disc_file_grouping`).
//!
//! Every file read goes through `content_file_source` (the OS by default; the web
//! edition installs its File System Access source around a call), so the same code
//! gives the same `[ID]` on both sides.

#![deny(unsafe_code)]

pub mod clrmamepro_dat_parser;
mod clrmamepro_game_blocks;
mod dat_facts;
pub mod dat_index;
mod dat_load_error;
mod dat_serial_index;
pub mod disc_file_grouping;
pub mod disc_game_identity;
pub mod game_content_sha1;
mod game_identity;
mod game_metadata_lookup;
pub mod gamecube_wii_identity;
mod metadata_dat_index;
mod metadata_record;
pub mod nds_game_code;
pub mod rom_archive;
pub mod rom_file_hasher;
pub mod serial_match_keys;
mod state_content_tag;
pub mod zip_scan;

pub use clrmamepro_dat_parser::{
    parse_clrmamepro_dat, parse_clrmamepro_games, DatEntry, DatGame, DatRom,
};
pub use dat_facts::{dat_facts, DatFacts};
pub use dat_index::{display_region, display_title, DatIndex};
pub use dat_load_error::DatLoadError;
pub use disc_file_grouping::{group_disc_files, DiscGroupCandidate, DiscGrouping};
pub use game_content_sha1::game_content_sha1;
pub use game_identity::{game_identity, match_dat, DatMatch, GameIdentity, IdentityError};
pub use game_metadata_lookup::{scanned_game_metadata, stored_game_metadata};
pub use metadata_dat_index::{
    metadata_attribute_folders, MetadataIndex, METADATA_PARTIAL_VERSION, METADATA_SOURCE_VERSION,
};
pub use metadata_record::MetadataRecord;
pub use rom_archive::{is_rom_archive, read_rom_archive, ArchivedRom};
pub use rom_file_hasher::{
    hash_bytes, hash_file_streaming, hash_headerless, hash_rom_bytes, hash_rom_file, read_rom_file,
    RomHashes,
};
pub use state_content_tag::state_content_tag;
pub use zip_scan::{scan_zip, ZipScan};
