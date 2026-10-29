//! Shapes the launcher exchanges with `fetch_game_artwork`: which images to
//! fetch for a game and what happened to each, classified so the caller can
//! persist final results and skip transient ones.

use crate::ThumbnailFolder;

/// The two images a game can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtworkKind {
    /// Box art (cached as JPEG).
    Cover,
    /// Wide image for Home/Detail: in-game snap, title screen as fallback (PNG).
    Hero,
}

impl ArtworkKind {
    /// Stable lower-case id used in the DB and events: "cover" / "hero".
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Hero => "hero",
        }
    }

    /// Cache file name for `game_id`, relative to the artwork dir.
    pub fn file_name(self, game_id: i64) -> String {
        match self {
            Self::Cover => format!("{game_id}-cover.jpg"),
            Self::Hero => format!("{game_id}-hero.png"),
        }
    }

    /// Thumbnail folders to try, best first.
    pub(crate) fn folders(self) -> &'static [ThumbnailFolder] {
        match self {
            Self::Cover => &[ThumbnailFolder::Boxarts],
            Self::Hero => &[ThumbnailFolder::Snaps, ThumbnailFolder::Titles],
        }
    }
}

/// One game to fetch art for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameArtRequest {
    /// DB id; names the cache files.
    pub game_id: i64,
    /// The game's system: which libretro-thumbnails repo serves its art.
    pub system: &'static system_catalog::SystemSpec,
    /// No-Intro (DAT) name, e.g. "Chrono Trigger (USA)".
    pub no_intro_name: String,
    /// Kinds to (re)fetch; duplicates are ignored.
    pub kinds: Vec<ArtworkKind>,
}

/// Result for one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindOutcome {
    /// Written to `<artwork_dir>/<relative_path>`; `source_name` is the
    /// upstream file used, e.g. `Named_Snaps/Chrono Trigger (USA).png`.
    Cached {
        relative_path: String,
        source_name: String,
    },
    /// No thumbnail under any candidate name (persist; retry after a while).
    NotFound,
    /// Downloaded but unusable, or a permanent HTTP/disk error (persist).
    Failed(String),
    /// Offline / network trouble; do not persist, retry next job.
    Transient(String),
    /// The cancel flag was set.
    Cancelled,
}

/// Results for one game, in requested order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtworkOutcome {
    /// One entry per distinct requested kind.
    pub results: Vec<(ArtworkKind, KindOutcome)>,
    /// True when at least one request completed a network round-trip (any
    /// HTTP answer). False for index-only "not found" and for transient or
    /// offline failures: the launcher's transient breaker resets only on true.
    pub network: bool,
}

impl ArtworkOutcome {
    /// Outcome for `kind`, if it was requested.
    pub fn get(&self, kind: ArtworkKind) -> Option<&KindOutcome> {
        self.results
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, outcome)| outcome)
    }

    /// True when every requested kind ended `Transient` (breaker input).
    pub fn is_transient(&self) -> bool {
        !self.results.is_empty()
            && self
                .results
                .iter()
                .all(|(_, outcome)| matches!(outcome, KindOutcome::Transient(_)))
    }
}
