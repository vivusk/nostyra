//! Owned Rust copies of rc_client data (achievements, leaderboards, user, game), so
//! nothing outside `ffi/` ever holds a pointer into rcheevos memory.

/// One achievement as rc_client reports it (`rc_client_achievement_t`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AchievementInfo {
    pub id: u32,
    pub title: String,
    pub description: String,
    /// Badge image name (`<name>.png` / `<name>_lock.png` on the media server).
    pub badge_name: String,
    /// Progress text such as `"5/10"`; empty when the achievement has no measure.
    pub measured_progress: String,
    pub measured_percent: f32,
    pub points: u32,
    /// Unix seconds; 0 while locked.
    pub unlock_time: i64,
    /// `RC_CLIENT_ACHIEVEMENT_STATE_*` (0 inactive, 1 active, 2 unlocked, 3 disabled).
    pub state: u8,
    /// `RC_CLIENT_ACHIEVEMENT_CATEGORY_*` (1 core, 2 unofficial).
    pub category: u8,
    /// `RC_CLIENT_ACHIEVEMENT_BUCKET_*`.
    pub bucket: u8,
    /// `RC_CLIENT_ACHIEVEMENT_UNLOCKED_*` bit set (1 softcore, 2 hardcore).
    pub unlocked: u8,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    /// `RC_CLIENT_ACHIEVEMENT_TYPE_*` (0 standard, 1 missable, 2 progression, 3 win).
    pub kind: u8,
}

/// One leaderboard (`rc_client_leaderboard_t`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeaderboardInfo {
    pub id: u32,
    pub title: String,
    pub description: String,
    /// Formatted current value while tracking; empty otherwise.
    pub tracker_value: String,
    /// `RC_CLIENT_LEADERBOARD_STATE_*`.
    pub state: u8,
    /// `RC_CLIENT_LEADERBOARD_FORMAT_*` (0 time, 1 score, 2 value).
    pub format: u8,
    pub lower_is_better: bool,
}

/// One group of an achievement list (`rc_client_achievement_bucket_t`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AchievementBucket {
    pub label: String,
    pub subset_id: u32,
    pub bucket_type: u8,
    pub achievements: Vec<AchievementInfo>,
}

/// One group of a leaderboard list (`rc_client_leaderboard_bucket_t`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeaderboardBucket {
    pub label: String,
    pub subset_id: u32,
    pub bucket_type: u8,
    pub leaderboards: Vec<LeaderboardInfo>,
}

/// One row of a post-submit scoreboard.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScoreboardEntry {
    pub username: String,
    pub rank: u32,
    pub score: String,
}

/// Ranking received after a leaderboard submit (`rc_client_leaderboard_scoreboard_t`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scoreboard {
    pub leaderboard_id: u32,
    pub submitted_score: String,
    pub best_score: String,
    pub new_rank: u32,
    pub num_entries: u32,
    pub top_entries: Vec<ScoreboardEntry>,
}

/// Logged-in user (token deliberately left out: it lives only in the credentials file).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserInfo {
    pub display_name: String,
    pub username: String,
    pub score: u32,
    pub score_softcore: u32,
}

/// Loaded (or unknown) game. `id == 0` with a non-empty `hash` = hash not in RA's database.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameInfo {
    pub id: u32,
    pub console_id: u32,
    pub title: String,
    pub hash: String,
    pub badge_name: String,
}

/// Unlock totals for the loaded game (`rc_client_user_game_summary_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameSummary {
    pub num_core_achievements: u32,
    pub num_unofficial_achievements: u32,
    pub num_unlocked_achievements: u32,
    pub num_unsupported_achievements: u32,
    pub points_core: u32,
    pub points_unlocked: u32,
}

/// `RC_CLIENT_LOAD_GAME_STATE_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadGameState {
    None,
    AwaitLogin,
    IdentifyingGame,
    FetchingGameData,
    StartingSession,
    Done,
    Aborted,
}

impl LoadGameState {
    /// Maps the C enum value; unknown values read as `None`.
    pub fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::AwaitLogin,
            2 => Self::IdentifyingGame,
            3 => Self::FetchingGameData,
            4 => Self::StartingSession,
            5 => Self::Done,
            6 => Self::Aborted,
            _ => Self::None,
        }
    }
}
