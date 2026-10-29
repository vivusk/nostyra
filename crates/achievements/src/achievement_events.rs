//! What rc_client tells the session, as owned Rust values: runtime events (copied out
//! of `rc_client_event_t` inside the event handler) and the results of the async
//! `begin_*` calls (login, game load, media change). Both are queued on the client and
//! drained by the session thread after `do_frame` / `idle` / request completion.

use crate::achievement_info::{AchievementInfo, LeaderboardInfo, Scoreboard};

/// One `rc_client_event_t`, fully copied (no pointers into rcheevos memory).
#[derive(Debug, Clone, PartialEq)]
pub enum AchievementEvent {
    AchievementTriggered(AchievementInfo),
    LeaderboardStarted(LeaderboardInfo),
    LeaderboardFailed(LeaderboardInfo),
    LeaderboardSubmitted(LeaderboardInfo),
    ChallengeIndicatorShow(AchievementInfo),
    ChallengeIndicatorHide(AchievementInfo),
    ProgressIndicatorShow(AchievementInfo),
    ProgressIndicatorHide,
    ProgressIndicatorUpdate(AchievementInfo),
    /// Tracker `id` appeared with formatted `display` text.
    TrackerShow {
        id: u32,
        display: String,
    },
    TrackerHide {
        id: u32,
    },
    TrackerUpdate {
        id: u32,
        display: String,
    },
    Scoreboard(Scoreboard),
    /// rc_client asks for a core reset (hardcore was enabled mid-session).
    Reset,
    GameCompleted,
    /// A request failed with a non-retryable server error.
    ServerError {
        api: String,
        message: String,
        result: i32,
        related_id: u32,
    },
    /// An unlock / submit could not be delivered and is being retried.
    Disconnected,
    /// Every pending unlock / submit has been delivered.
    Reconnected,
    SubsetCompleted {
        id: u32,
        title: String,
    },
}

/// Which async rc_client call a [`ClientCompletion`] answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOp {
    LoginWithToken,
    LoadGame,
    ChangeMedia,
}

impl ClientOp {
    /// Encodes the op as the callback userdata (a small integer, never a real pointer,
    /// so a callback rc_client delivers after destroy can never dereference freed data).
    pub(crate) fn as_tag(self) -> usize {
        match self {
            Self::LoginWithToken => 1,
            Self::LoadGame => 2,
            Self::ChangeMedia => 3,
        }
    }

    pub(crate) fn from_tag(tag: usize) -> Option<Self> {
        match tag {
            1 => Some(Self::LoginWithToken),
            2 => Some(Self::LoadGame),
            3 => Some(Self::ChangeMedia),
            _ => None,
        }
    }
}

/// `RC_OK`.
pub const RC_OK: i32 = 0;
/// `RC_LOGIN_REQUIRED`: a game load while nobody is logged in (the login failed).
pub const RC_LOGIN_REQUIRED: i32 = -28;
/// `RC_NO_GAME_LOADED`: unknown hash (game info still carries the hash) or no game.
pub const RC_NO_GAME_LOADED: i32 = -29;
/// `RC_HARDCORE_DISABLED`: a disc swap inserted unknown media; hardcore is now off.
pub const RC_HARDCORE_DISABLED: i32 = -30;
/// `RC_NO_RESPONSE`: the request got status −1/−2 (timeout, offline, shutdown).
pub const RC_NO_RESPONSE: i32 = -32;
/// `RC_ACCESS_DENIED`.
pub const RC_ACCESS_DENIED: i32 = -33;
/// `RC_INVALID_CREDENTIALS`: wrong password / token → log in again.
pub const RC_INVALID_CREDENTIALS: i32 = -34;
/// `RC_EXPIRED_TOKEN`: token no longer valid → log in again.
pub const RC_EXPIRED_TOKEN: i32 = -35;

/// Result of one `begin_*` call (`rc_client_callback_t`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCompletion {
    pub op: ClientOp,
    /// `RC_OK` or an `RC_*` error code.
    pub result: i32,
    /// rcheevos error text (empty on success).
    pub message: String,
}

impl ClientCompletion {
    /// True for success.
    pub fn is_ok(&self) -> bool {
        self.result == RC_OK
    }

    /// True when the server rejected the stored token (user must log in again).
    pub fn is_login_rejected(&self) -> bool {
        matches!(self.result, RC_INVALID_CREDENTIALS | RC_EXPIRED_TOKEN)
    }
}
