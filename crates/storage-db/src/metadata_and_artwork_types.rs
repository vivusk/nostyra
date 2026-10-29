//! M2 data types: game metadata (index-derived or user-edited), artwork records and
//! the composed game detail.

use serde::{Deserialize, Serialize};

use crate::domain_types::{Game, PlayStats};

/// Index-derived metadata of one game (vendored libretro-database metadat).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameMetadataRecord {
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<u16>,
    pub players: Option<u8>,
}

/// Metadata attached to a scanned game, with the metadat version that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedMetadata {
    pub record: GameMetadataRecord,
    pub source_version: &'static str,
}

/// A stored `game_metadata` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMetadata {
    pub title_override: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<i64>,
    pub players: Option<i64>,
    pub description: Option<String>,
    /// Saved by the user: frozen against metadat refreshes until reset.
    pub user_edited: bool,
    pub updated_at: i64,
}

/// User edit from the Edit metadata dialog; blank strings mean "no value".
/// Validated and length-capped by `update_user_metadata`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserMetadataEdit {
    pub title: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<i64>,
    pub players: Option<i64>,
    pub description: Option<String>,
}

/// Outcome of an artwork lookup that is worth remembering (transient failures are not).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtworkStatus {
    Cached,
    NotFound,
    Failed,
}

impl ArtworkStatus {
    /// Value stored in `artwork.status`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cached => "cached",
            Self::NotFound => "not-found",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "cached" => Self::Cached,
            "not-found" => Self::NotFound,
            _ => Self::Failed,
        }
    }
}

/// One `artwork` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkRecord {
    pub game_id: i64,
    /// `"cover"` | `"hero"`.
    pub kind: String,
    pub status: ArtworkStatus,
    /// Relative to `<data>/artwork`; `None` unless cached.
    pub local_path: Option<String>,
    pub source: String,
    pub source_name: Option<String>,
    pub checked_at: i64,
}

/// A game to fetch artwork for: id + No-Intro name (the thumbnail lookup key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkWorkItem {
    pub game_id: i64,
    pub platform_id: String,
    pub dat_name: String,
}

/// Everything the Detail page shows from the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    #[serde(flatten)]
    pub game: Game,
    pub metadata: Option<GameMetadata>,
    pub stats: PlayStats,
}
