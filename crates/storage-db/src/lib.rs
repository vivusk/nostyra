//! Launcher storage: one SQLite file `<data>/nostyra.db` (WAL), append-only migrations,
//! and plain-function repositories over a `rusqlite::Connection`.
//!
//! The launcher is the only writer and keeps one connection behind a mutex; scans write
//! in small transactions so list queries never wait for a whole scan.

#![deny(unsafe_code)]

mod artwork_repository;
mod core_and_settings_repository;
mod database_connection;
mod domain_types;
mod game_list_sql;
mod game_metadata_repository;
mod game_queries;
mod game_removal;
mod game_scan_writes;
mod library_folder_repository;
mod metadata_and_artwork_types;
mod play_session_repository;
mod storage_error;
mod user_metadata_edits;

pub use artwork_repository::{
    clear_artwork, games_needing_artwork, get_artwork, uncached_artwork_kinds, upsert_artwork,
};
pub use core_and_settings_repository::{
    get_installed_core, get_setting, save_installed_core, set_setting,
};
pub use database_connection::{open_database, open_in_memory_database};
pub use domain_types::{
    FileFingerprint, Game, GameFilesKey, GameFilter, GameListQuery, GameSort, InstalledCoreRecord,
    LibraryFolder, LibrarySummary, NewPlaySession, PlayStats, RelinkedGame, ScannedGame,
    UpsertOutcome,
};
pub use game_metadata_repository::{
    delete_metadata, games_needing_metadata, get_game_detail, get_game_metadata, list_genres,
    set_favorite, upsert_from_index, MetadataWorkItem,
};
pub use game_queries::{
    find_game_id_by_rom_path, game_platform_ids, get_game, library_platform_counts,
    library_summary, list_games, RECENTLY_PLAYED_WINDOW_S,
};
pub use game_removal::{delete_missing_game, game_files_keys, visible_missing_game_ids};
pub use game_scan_writes::{file_fingerprints, mark_missing_paths, upsert_scanned_batch};
pub use library_folder_repository::{
    add_library_folder, list_library_folders, remove_library_folder,
};
pub use metadata_and_artwork_types::{
    ArtworkRecord, ArtworkStatus, ArtworkWorkItem, GameDetail, GameMetadata, GameMetadataRecord,
    ScannedMetadata, UserMetadataEdit,
};
pub use play_session_repository::{game_play_stats, record_play_session};
pub use rusqlite::Connection;
pub use storage_error::{StorageError, StorageResult};
pub use user_metadata_edits::update_user_metadata;
