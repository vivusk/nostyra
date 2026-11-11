//! Live side of the registry: WebSocket connects / disconnects and room GC.

use std::time::Instant;

use matchbox_protocol::PeerId;

use crate::room_model::RegistryError;
use crate::room_registry::RoomRegistry;
use crate::seat_role::SeatRole;

impl RoomRegistry {
    /// WebSocket upgrade for a verified token: maps its seat to the matchbox id and
    /// enforces the live caps (a stale seat cannot push the room over `max_players`).
    pub fn connect(
        &self,
        room_id: &str,
        peer_id: &str,
        live: PeerId,
        now: Instant,
    ) -> Result<SeatRole, RegistryError> {
        let mut rooms = self.lock();
        let max_spectators = self.limits.max_spectators;
        let room = rooms
            .by_id
            .get_mut(room_id)
            .ok_or(RegistryError::NotFound)?;
        if room.is_expired(now) {
            return Err(RegistryError::Expired);
        }
        let role = match room.seats.get(peer_id) {
            None => return Err(RegistryError::UnknownSeat),
            Some(seat) if seat.live.is_some() => return Err(RegistryError::AlreadyConnected),
            Some(seat) => seat.role,
        };
        let cap = if role.is_player() {
            usize::from(room.max_players)
        } else {
            max_spectators
        };
        if room.live_count(role.is_player()) >= cap {
            return Err(RegistryError::RoomFull);
        }
        if let Some(seat) = room.seats.get_mut(peer_id) {
            seat.live = Some(live);
        }
        room.ever_connected = true;
        room.empty_since = None;
        Ok(role)
    }

    /// WebSocket closed (or the upgrade failed after `connect`).
    pub fn disconnect(&self, room_id: &str, peer_id: &str, now: Instant) {
        let mut rooms = self.lock();
        let Some(room) = rooms.by_id.get_mut(room_id) else {
            return;
        };
        if let Some(seat) = room.seats.get_mut(peer_id) {
            seat.live = None;
            seat.left = true;
        }
        if room.live_total() == 0 {
            room.empty_since = Some(now);
        }
    }

    /// Removes dead rooms; returns how many went.
    pub fn collect_garbage(&self, now: Instant) -> usize {
        let mut rooms = self.lock();
        let dead: Vec<String> = rooms
            .by_id
            .iter()
            .filter(|(_, room)| room.is_dead(now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &dead {
            if let Some(room) = rooms.by_id.remove(id) {
                rooms.id_by_code.remove(&room.code);
            }
        }
        dead.len()
    }
}

#[cfg(test)]
#[path = "room_presence_tests.rs"]
mod tests;
