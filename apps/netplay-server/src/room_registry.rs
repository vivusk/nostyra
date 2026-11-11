//! In-memory rooms: codes, seats (issued tokens), live connections, caps and GC.
//! Every method takes `now` so tests drive the clock. Locks are held only for map work.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use crate::random_ids::random_hex;
use crate::room_code::generate_room_code;
use crate::room_model::*;
use crate::room_token::TOKEN_TTL;
use crate::seat_role::SeatRole;

/// Caps from the server config.
#[derive(Debug, Clone, Copy)]
pub struct RoomLimits {
    pub max_rooms: usize,
    pub max_spectators: usize,
}

#[derive(Default)]
pub(crate) struct Rooms {
    pub(crate) by_id: HashMap<String, Room>,
    pub(crate) id_by_code: HashMap<String, String>,
}

/// Shared by the HTTP handlers, the WebSocket handler and the GC task.
pub struct RoomRegistry {
    rooms: Mutex<Rooms>,
    pub(crate) limits: RoomLimits,
}

impl RoomRegistry {
    pub fn new(limits: RoomLimits) -> Self {
        Self {
            rooms: Mutex::new(Rooms::default()),
            limits,
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Rooms> {
        // A panic while holding the lock leaves plain maps behind: keep serving.
        self.rooms
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Number of rooms currently held.
    pub fn room_count(&self) -> usize {
        self.lock().by_id.len()
    }

    /// Host creates a room; the host seat is issued with it.
    pub fn create(&self, req: NewRoom, now: Instant) -> Result<SeatGrant, RegistryError> {
        let mut rooms = self.lock();
        if rooms.by_id.len() >= self.limits.max_rooms {
            return Err(RegistryError::TooManyRooms);
        }
        let room_id = random_hex(16).map_err(|_| RegistryError::Internal)?;
        let code = (0..16)
            .filter_map(|_| generate_room_code().ok())
            .find(|c| !rooms.id_by_code.contains_key(c))
            .ok_or(RegistryError::Internal)?;
        let mut room = Room {
            code: code.clone(),
            game: req.game,
            max_players: req.max_players,
            allow_spectators: req.allow_spectators,
            started: false,
            created_at: now,
            ever_connected: false,
            empty_since: None,
            seats: HashMap::new(),
        };
        let grant = issue_seat(&room_id, &mut room, req.nickname, SeatRole::Host, now)?;
        rooms.id_by_code.insert(code, room_id.clone());
        rooms.by_id.insert(room_id, room);
        Ok(grant)
    }

    /// Player or spectator joins by code (before `/start` only).
    pub fn join(
        &self,
        code: &str,
        nickname: String,
        role: SeatRole,
        now: Instant,
    ) -> Result<SeatGrant, RegistryError> {
        let mut rooms = self.lock();
        let room_id = rooms
            .id_by_code
            .get(code)
            .cloned()
            .ok_or(RegistryError::NotFound)?;
        let room = rooms
            .by_id
            .get_mut(&room_id)
            .ok_or(RegistryError::NotFound)?;
        if room.is_expired(now) {
            return Err(RegistryError::Expired);
        }
        if room.started {
            return Err(RegistryError::Started);
        }
        // Seats whose token expired can never connect again: forget them.
        room.seats.retain(|_, seat| {
            seat.live.is_some() || now.duration_since(seat.issued_at) < TOKEN_TTL
        });
        let full = match role {
            SeatRole::Host => return Err(RegistryError::NotHost),
            SeatRole::Spectator if !room.allow_spectators => {
                return Err(RegistryError::SpectatorsOff)
            }
            SeatRole::Spectator => room.placed_count(false, now) >= self.limits.max_spectators,
            SeatRole::Player => room.placed_count(true, now) >= usize::from(room.max_players),
        };
        if full {
            return Err(RegistryError::RoomFull);
        }
        issue_seat(&room_id, room, nickname, role, now)
    }

    /// Live counts for the lobby poll.
    pub fn status(&self, code: &str, now: Instant) -> Result<RoomStatus, RegistryError> {
        let rooms = self.lock();
        let room = rooms
            .id_by_code
            .get(code)
            .and_then(|id| rooms.by_id.get(id))
            .ok_or(RegistryError::NotFound)?;
        if room.is_expired(now) && room.live_total() == 0 {
            return Err(RegistryError::Expired);
        }
        Ok(RoomStatus {
            players: room.live_count(true),
            spectators: room.live_count(false),
            started: room.started,
            max_players: room.max_players,
        })
    }

    /// Host marks the room started; later joins get 409. `room_id` / `role` come from
    /// the verified token: a token of another room or a non-host seat is refused.
    pub fn start(&self, code: &str, room_id: &str, role: SeatRole) -> Result<(), RegistryError> {
        let mut rooms = self.lock();
        let code_room = rooms.id_by_code.get(code).ok_or(RegistryError::NotFound)?;
        if code_room != room_id || role != SeatRole::Host {
            return Err(RegistryError::NotHost);
        }
        let room = rooms
            .by_id
            .get_mut(room_id)
            .ok_or(RegistryError::NotFound)?;
        room.started = true;
        Ok(())
    }
}

fn issue_seat(
    room_id: &str,
    room: &mut Room,
    nickname: String,
    role: SeatRole,
    now: Instant,
) -> Result<SeatGrant, RegistryError> {
    let peer_id = random_hex(8).map_err(|_| RegistryError::Internal)?;
    let seat = Seat {
        nickname,
        role,
        issued_at: now,
        live: None,
        left: false,
    };
    room.seats.insert(peer_id.clone(), seat);
    Ok(SeatGrant {
        room_id: room_id.to_owned(),
        code: room.code.clone(),
        peer_id,
        role,
        game: room.game.clone(),
        max_players: room.max_players,
        roster: room.roster(now),
    })
}

#[cfg(test)]
#[path = "room_registry_tests.rs"]
pub(crate) mod tests;
