//! Data kept per room (in memory only, nothing persisted) and the registry's results.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use matchbox_protocol::PeerId;
use serde::{Deserialize, Serialize};

use crate::seat_role::SeatRole;

/// A room accepts joins / connects for this long after creation.
pub const ROOM_LIFETIME: Duration = Duration::from_secs(2 * 60 * 60);
/// A room nobody ever connected to is removed after this grace (host cloud pull,
/// dialogs, runner launch).
pub const NEVER_CONNECTED_GRACE: Duration = Duration::from_secs(5 * 60);
/// Once someone connected, a room without live peers is removed after this.
pub const EMPTY_ROOM_GRACE: Duration = Duration::from_secs(60);
/// An issued seat that has not connected holds its place this long.
pub const PENDING_SEAT_TTL: Duration = Duration::from_secs(5 * 60);

/// Lobby hint from the host: the joiner's library lookup and error text. Compatibility
/// is decided by the runner handshake, never here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameHint {
    pub content_sha1: String,
    pub title: String,
    pub system: String,
    pub core_id: String,
    pub core_version: String,
}

/// One issued token (keyed by its `peer_id`).
#[derive(Debug, Clone)]
pub(crate) struct Seat {
    pub nickname: String,
    pub role: SeatRole,
    pub issued_at: Instant,
    /// matchbox id while the WebSocket is open.
    pub live: Option<PeerId>,
    /// Connected once and left: no longer holds a place for joins.
    pub left: bool,
}

impl Seat {
    /// Counts against the room's caps for new joins.
    pub fn holds_place(&self, now: Instant) -> bool {
        self.live.is_some() || (!self.left && now.duration_since(self.issued_at) < PENDING_SEAT_TTL)
    }
}

#[derive(Debug)]
pub(crate) struct Room {
    pub code: String,
    pub game: GameHint,
    pub max_players: u8,
    pub allow_spectators: bool,
    pub started: bool,
    pub created_at: Instant,
    pub ever_connected: bool,
    pub empty_since: Option<Instant>,
    pub seats: HashMap<String, Seat>,
}

impl Room {
    pub fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.created_at) >= ROOM_LIFETIME
    }

    pub fn live_count(&self, players: bool) -> usize {
        self.seats
            .values()
            .filter(|s| s.live.is_some() && s.role.is_player() == players)
            .count()
    }

    pub fn placed_count(&self, players: bool, now: Instant) -> usize {
        self.seats
            .values()
            .filter(|s| s.holds_place(now) && s.role.is_player() == players)
            .count()
    }

    pub fn live_total(&self) -> usize {
        self.seats.values().filter(|s| s.live.is_some()).count()
    }

    /// Seats that hold a place, for the join response.
    pub fn roster(&self, now: Instant) -> Vec<RosterEntry> {
        let entry = |s: &Seat| RosterEntry {
            nickname: s.nickname.clone(),
            role: s.role,
            connected: s.live.is_some(),
        };
        self.seats
            .values()
            .filter(|s| s.holds_place(now))
            .map(entry)
            .collect()
    }

    /// GC rule: never connected → 5 min after creation; connected once → 60 s without
    /// live peers; expired (2 h) → as soon as nobody is connected (a running game keeps
    /// its room until everyone left).
    pub fn is_dead(&self, now: Instant) -> bool {
        if self.live_total() > 0 {
            return false;
        }
        if self.is_expired(now) {
            return true;
        }
        if !self.ever_connected {
            return now.duration_since(self.created_at) >= NEVER_CONNECTED_GRACE;
        }
        self.empty_since
            .is_some_and(|t| now.duration_since(t) >= EMPTY_ROOM_GRACE)
    }
}

/// Host request for a new room.
#[derive(Debug, Clone)]
pub struct NewRoom {
    pub nickname: String,
    pub game: GameHint,
    pub max_players: u8,
    pub allow_spectators: bool,
}

/// A seat handed out by create / join: the API signs a token for it.
#[derive(Debug, Clone)]
pub struct SeatGrant {
    pub room_id: String,
    pub code: String,
    pub peer_id: String,
    pub role: SeatRole,
    pub game: GameHint,
    pub max_players: u8,
    pub roster: Vec<RosterEntry>,
}

/// Lobby roster line (no ids, no addresses).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RosterEntry {
    pub nickname: String,
    pub role: SeatRole,
    pub connected: bool,
}

/// `GET /rooms/{code}`: counts from live WebSocket connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RoomStatus {
    pub players: usize,
    pub spectators: usize,
    pub started: bool,
    pub max_players: u8,
}

/// Registry refusals; the API maps each to one HTTP status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    NotFound,
    Expired,
    RoomFull,
    Started,
    SpectatorsOff,
    TooManyRooms,
    NotHost,
    UnknownSeat,
    AlreadyConnected,
    Internal,
}
