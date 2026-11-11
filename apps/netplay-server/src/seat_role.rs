//! Role of a seat in a room: fixed when the token is issued.

use serde::{Deserialize, Serialize};

/// Host = the player who created the room (only one allowed to `/start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeatRole {
    Host,
    Player,
    Spectator,
}

impl SeatRole {
    /// Wire / token spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Player => "player",
            Self::Spectator => "spectator",
        }
    }

    /// Inverse of [`SeatRole::as_str`].
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "host" => Some(Self::Host),
            "player" => Some(Self::Player),
            "spectator" => Some(Self::Spectator),
            _ => None,
        }
    }

    /// Host and players share the `max_players` cap; spectators have their own.
    pub fn is_player(self) -> bool {
        !matches!(self, Self::Spectator)
    }
}
