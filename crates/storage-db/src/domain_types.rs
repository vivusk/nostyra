//! Plain data types crossing the storage boundary (serde for Tauri commands).
//!
//! Paths are stored as UTF-8 strings; callers skip non-UTF-8 paths before writing.
//! Times are unix seconds.

use serde::{Deserialize, Serialize};

use crate::metadata_and_artwork_types::ScannedMetadata;

/// A user-chosen ROM folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
}

/// One hashed ROM file from a scan, ready to upsert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedGame {
    pub platform_id: String,
    pub folder_id: i64,
    pub rom_path: String,
    pub file_size: i64,
    pub file_mtime_ns: i64,
    /// Upper-case hex of the headerless image.
    pub crc32: String,
    /// Upper-case hex of the headerless image; also the relink key for moved files.
    pub sha1: String,
    pub title: String,
    pub region: Option<String>,
    /// Full DAT entry name; `None` = not identified ("needs review").
    pub dat_name: Option<String>,
    /// Vendored metadat for DAT-matched games; `None` when the index has nothing.
    pub metadata: Option<ScannedMetadata>,
    /// Disc boot serial (`SLUS-01234`) or the DS game code (`AMCE`); `None` for other cartridges.
    pub serial: Option<String>,
}

/// What a batch upsert did, for the scan summary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpsertOutcome {
    pub inserted: usize,
    pub updated: usize,
    /// Existing missing rows taken over by a moved/renamed file (play history kept).
    pub relinked: Vec<RelinkedGame>,
}

/// A missing row taken over by a moved/renamed file. The launcher moves the game's
/// save folder and config when the ROM key (file stem) changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelinkedGame {
    pub game_id: i64,
    pub platform_id: String,
    pub old_rom_path: String,
    pub new_rom_path: String,
}

/// The stored facts a game's own files are named by (`<rom key> [<sha1 prefix>]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFilesKey {
    pub game_id: i64,
    pub platform_id: String,
    pub rom_path: String,
    pub sha1: String,
}

/// Size + mtime of a file as last scanned: unchanged files are not re-hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFingerprint {
    pub size: i64,
    pub mtime_ns: i64,
}

/// A game as listed in the library, with its play stats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub platform_id: String,
    pub folder_id: Option<i64>,
    pub rom_path: String,
    pub file_size: i64,
    pub crc32: String,
    pub sha1: String,
    /// Display title: the user's title override, else the scanned title.
    pub title: String,
    pub region: Option<String>,
    pub dat_name: Option<String>,
    pub added_at: i64,
    /// File not found on the last scan: shown greyed, Play disabled.
    pub missing: bool,
    pub total_play_s: i64,
    pub last_played_at: Option<i64>,
    pub genre: Option<String>,
    pub release_year: Option<i64>,
    pub developer: Option<String>,
    pub favorite: bool,
    /// Cached cover / hero file, relative to `<data>/artwork` (launcher makes it absolute).
    pub cover_path: Option<String>,
    pub hero_path: Option<String>,
    /// `checked_at` of the cached cover: changes only when the file is rewritten (UI cache-bust).
    pub cover_checked_at: Option<i64>,
    /// Disc boot serial (`SLUS-01234`) or the DS game code (`AMCE`); `None` for other cartridges.
    pub serial: Option<String>,
}

impl Game {
    /// Not matched against the DAT.
    pub fn needs_review(&self) -> bool {
        self.dat_name.is_none()
    }
}

/// Library list ordering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameSort {
    /// Most recent session first; never-played games last (A–Z).
    #[default]
    RecentlyPlayed,
    Title,
    /// Most total play time first.
    PlayTime,
    /// Newest `added_at` first.
    RecentlyAdded,
}

/// Library list filter (genre is a separate field of [`GameListQuery`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameFilter {
    #[default]
    All,
    /// Played within [`crate::RECENTLY_PLAYED_WINDOW_S`].
    RecentlyPlayed,
    Unplayed,
    NeedsReview,
    Favorites,
    /// ROM file not found on the last scan.
    Missing,
    /// Played at least once, any time (Home "Continue playing"; not a Library pill).
    Played,
}

/// Library list query from the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameListQuery {
    /// Case-insensitive substring of the title; blank = no search.
    pub search: Option<String>,
    pub sort: GameSort,
    pub filter: GameFilter,
    /// Exact genre; `None` = any.
    pub genre: Option<String>,
    /// Platform id (system filter); `None` = every system.
    pub platform: Option<String>,
    /// Maximum rows; `None` = all.
    pub limit: Option<u32>,
}

/// Header counts: "N games · M matched · K need review".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    pub total: i64,
    pub matched: i64,
    pub needs_review: i64,
    /// ROM file not found on the last scan.
    pub missing: i64,
}

/// A finished play session to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlaySession {
    pub game_id: i64,
    pub started_at: i64,
    pub duration_s: i64,
    pub exit_code: Option<i32>,
    /// Pending-session file name for runner-written sessions; `None` for launcher-measured ones.
    pub source_key: Option<String>,
}

/// Aggregated play time of one game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStats {
    pub total_play_s: i64,
    pub last_played_at: Option<i64>,
    pub session_count: i64,
}

/// A downloaded, verified libretro core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCoreRecord {
    pub id: String,
    pub platform_id: String,
    pub library_path: String,
    pub version: Option<String>,
    pub sha256: String,
    pub installed_at: i64,
}
