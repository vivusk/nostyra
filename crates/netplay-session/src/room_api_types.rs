//! JSON shapes of the signaling server's `/v1/rooms*` API (`apps/netplay-server`
//! `rooms_http_api.rs`). Secrets (tokens, room ids, room codes, TURN credentials)
//! never appear in `Debug` output.

use serde::{Deserialize, Serialize};

/// Seat role, fixed when the token is issued (server spelling: lowercase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeatRole {
    /// Created the room; only one allowed to `/start`; ggrs handle 0.
    Host,
    Player,
    Spectator,
}

impl SeatRole {
    /// Host and players share the `max_players` cap; spectators have their own.
    pub fn is_player(self) -> bool {
        !matches!(self, Self::Spectator)
    }
}

/// One STUN / TURN entry. TURN entries carry a time-limited username / credential.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl IceServer {
    /// True when any url is a `turn:` / `turns:` url.
    pub fn is_turn(&self) -> bool {
        self.urls.iter().any(|url| url.starts_with("turn"))
    }
}

impl std::fmt::Debug for IceServer {
    /// Url count and kind only: TURN urls name the server, the credential is a secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IceServer")
            .field("urls", &self.urls.len())
            .field("turn", &self.is_turn())
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Lobby hint about the host's game (never a fingerprint: the runner checks that).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameHint {
    pub content_sha1: String,
    pub title: String,
    pub system: String,
    pub core_id: String,
    pub core_version: String,
}

/// `POST /v1/rooms` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateRoomRequest {
    pub nickname: String,
    pub game: GameHint,
    /// 2..=4 (server refuses others).
    pub max_players: u8,
    pub allow_spectators: bool,
}

/// `POST /v1/rooms` reply.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct CreateRoomResponse {
    pub code: String,
    pub room_id: String,
    pub token: String,
    pub ice_servers: Vec<IceServer>,
    /// Token expiry (Unix seconds): the runner must open the WebSocket before it.
    pub expires_at: u64,
}

/// `POST /v1/rooms/{code}/join` body (`role` = player or spectator).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinRequest {
    pub nickname: String,
    pub role: SeatRole,
}

/// Lobby roster line from the server (no ids, no addresses).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RoomRosterEntry {
    pub nickname: String,
    pub role: SeatRole,
    pub connected: bool,
}

/// `POST /v1/rooms/{code}/join` reply.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct JoinResponse {
    pub room_id: String,
    pub token: String,
    pub ice_servers: Vec<IceServer>,
    pub roster: Vec<RoomRosterEntry>,
    pub game: GameHint,
    /// The room's player cap: the guest applies the same multitap (F21).
    pub max_players: u8,
    pub expires_at: u64,
}

/// `GET /v1/rooms/{code}` reply (counts of live connections).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct RoomStatus {
    pub players: usize,
    pub spectators: usize,
    pub started: bool,
    pub max_players: u8,
}

impl std::fmt::Debug for CreateRoomResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateRoomResponse")
            .field("code", &"<redacted>")
            .field("token", &"<redacted>")
            .field("ice_servers", &self.ice_servers)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for JoinResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinResponse")
            .field("token", &"<redacted>")
            .field("ice_servers", &self.ice_servers)
            .field("roster", &self.roster)
            .field("game", &self.game)
            .field("max_players", &self.max_players)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn join_reply_parses_and_debug_hides_secrets() {
        let json = r#"{"room_id":"r1","token":"tok.sig","ice_servers":[
            {"urls":["stun:h:3478"]},
            {"urls":["turn:h:3478?transport=udp"],"username":"9:p","credential":"SECRETCRED"}],
            "roster":[{"nickname":"Ann","role":"host","connected":true}],
            "game":{"content_sha1":"ab","title":"T","system":"s","core_id":"c","core_version":"1"},
            "max_players":4,"expires_at":99}"#;
        let reply: JoinResponse = serde_json::from_str(json).unwrap();
        assert_eq!(reply.max_players, 4);
        assert_eq!(reply.roster[0].role, SeatRole::Host);
        assert!(reply.ice_servers[1].is_turn() && !reply.ice_servers[0].is_turn());
        let text = format!("{reply:?}");
        for secret in ["tok.sig", "SECRETCRED", "9:p", "turn:h", "r1"] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
    }

    #[test]
    fn roles_use_the_server_spelling() {
        let text = serde_json::to_string(&SeatRole::Spectator).unwrap();
        assert_eq!(text, "\"spectator\"");
        assert!(SeatRole::Host.is_player() && !SeatRole::Spectator.is_player());
    }
}
