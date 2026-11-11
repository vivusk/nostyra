//! Host side, lobby part: greet or refuse joiners (F22 late join, F28 caps),
//! roster updates, peers leaving, lobby pings (RTT for the input delay).

use std::time::Instant;

use super::{EndReason, LobbyEffect, LobbyPhase, LobbyState};
use super::{MAX_SPECTATORS, PING_INTERVAL};
use crate::lobby_protocol::{LobbyMember, LobbyMessage, RefusalReason};
use crate::room_api_types::SeatRole;
use crate::session_fingerprint::SessionFingerprint;
use crate::PeerKey;

impl LobbyState {
    pub(super) fn on_host_message(
        &mut self,
        peer: PeerKey,
        msg: LobbyMessage,
        now: Instant,
        out: &mut Vec<LobbyEffect>,
    ) {
        match msg {
            LobbyMessage::Hello {
                fingerprint,
                nickname,
                role,
            } => {
                self.on_hello(peer, &fingerprint, nickname, role, out);
            }
            LobbyMessage::SyncAck if self.phase == LobbyPhase::Syncing => {
                self.acks.insert(peer);
                self.check_acks(out);
            }
            LobbyMessage::ResyncReady if self.phase == LobbyPhase::Resyncing => {
                self.acks.insert(peer);
                self.check_acks(out);
            }
            LobbyMessage::Pong { nonce } => self.on_pong(peer, nonce, now, out),
            _ => {}
        }
    }

    fn on_hello(
        &mut self,
        peer: PeerKey,
        fp: &SessionFingerprint,
        nickname: String,
        role: SeatRole,
        out: &mut Vec<LobbyEffect>,
    ) {
        if self.roster.iter().any(|m| m.peer == peer) || self.phase == LobbyPhase::Connecting {
            return; // duplicate Hello
        }
        let reason = if self.phase != LobbyPhase::InLobby {
            Some(RefusalReason::Started)
        } else if let diff @ [_, ..] = self.cfg.fingerprint.diff(fp).as_slice() {
            Some(RefusalReason::Mismatch(diff.to_vec()))
        } else if self.is_full(role) {
            Some(RefusalReason::RoomFull)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.refused.insert(peer);
            return self.send(vec![peer], LobbyMessage::Refused { reason }, out);
        }
        let role = if role == SeatRole::Host {
            SeatRole::Player
        } else {
            role
        };
        let players = self.roster.iter().filter(|m| m.role.is_player()).count();
        let handle = role.is_player().then_some(players);
        let nickname: String = nickname
            .chars()
            .filter(|c| !c.is_control())
            .take(32)
            .collect();
        self.roster.push(LobbyMember {
            peer,
            nickname,
            role,
            handle,
            rtt_ms: None,
        });
        self.refused.remove(&peer);
        let welcome = LobbyMessage::Welcome {
            roster: self.roster.clone(),
            your_handle: handle,
        };
        self.send(vec![peer], welcome, out);
        let others: Vec<PeerKey> = self.guests().into_iter().filter(|&p| p != peer).collect();
        self.send(others, self.roster_update(), out);
        out.push(LobbyEffect::NotifyUi);
    }

    fn is_full(&self, role: SeatRole) -> bool {
        let count = |player: bool| {
            self.roster
                .iter()
                .filter(|m| m.role.is_player() == player)
                .count()
        };
        if role.is_player() {
            count(true) >= usize::from(self.cfg.fingerprint.max_players)
        } else {
            count(false) >= MAX_SPECTATORS
        }
    }

    pub(super) fn on_peer_left(&mut self, peer: PeerKey, out: &mut Vec<LobbyEffect>) {
        self.connected.remove(&peer);
        self.refused.remove(&peer);
        if !self.is_host() {
            return self.guest_peer_left(peer, out);
        }
        let before = self.roster.len();
        self.roster.retain(|m| m.peer != peer);
        self.acks.remove(&peer);
        if self.roster.len() != before {
            if self.phase == LobbyPhase::InLobby {
                self.renumber_players();
            }
            self.send(self.guests(), self.roster_update(), out);
            out.push(LobbyEffect::NotifyUi);
            self.check_acks(out);
        }
    }

    pub(super) fn on_tick(&mut self, now: Instant, out: &mut Vec<LobbyEffect>) {
        if self.resync_deadline.is_some_and(|deadline| now >= deadline) {
            return self.end(
                EndReason::Error("a peer did not answer the resync".into()),
                out,
            );
        }
        if !self.is_host() || self.phase != LobbyPhase::InLobby {
            return;
        }
        if self.next_ping.is_some_and(|at| now < at) {
            return;
        }
        self.next_ping = Some(now + PING_INTERVAL);
        self.pings
            .retain(|_, (_, sent)| now.saturating_duration_since(*sent) < PING_INTERVAL * 5);
        for peer in self.guests() {
            self.nonce = self.nonce.wrapping_add(1);
            self.pings.insert(self.nonce, (peer, now));
            self.send(vec![peer], LobbyMessage::Ping { nonce: self.nonce }, out);
        }
    }

    /// Lobby handles stay `0..players` in join order (`Start` assigns the same).
    fn renumber_players(&mut self) {
        let players = self.roster.iter_mut().filter(|m| m.role.is_player());
        players.enumerate().for_each(|(i, m)| m.handle = Some(i));
    }

    pub(super) fn roster_update(&self) -> LobbyMessage {
        LobbyMessage::RosterUpdate {
            roster: self.roster.clone(),
        }
    }

    fn on_pong(&mut self, peer: PeerKey, nonce: u64, now: Instant, out: &mut Vec<LobbyEffect>) {
        let Some((sent_to, sent)) = self.pings.remove(&nonce) else {
            return;
        };
        let Some(member) = self
            .roster
            .iter_mut()
            .find(|m| m.peer == peer && sent_to == peer)
        else {
            return;
        };
        let rtt = now.saturating_duration_since(sent).as_millis();
        member.rtt_ms = Some(u32::try_from(rtt).unwrap_or(u32::MAX));
        self.send(self.guests(), self.roster_update(), out);
        out.push(LobbyEffect::NotifyUi);
    }
}
