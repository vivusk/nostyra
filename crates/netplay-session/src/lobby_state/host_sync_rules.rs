//! Host side, session part: sync to every guest, `Start` after every `SyncAck`
//! (handles host-first, input delay from the worst lobby RTT), resync handshake
//! (`Resync` → every `ResyncReady` → the same sync path → `RebuildDriver`, F23).

use std::time::Instant;

use super::{
    EndReason, LobbyEffect, LobbyEvent, LobbyPhase, LobbyState, StartPlan, RESYNC_TIMEOUT,
};
use crate::lobby_protocol::LobbyMessage;
use crate::peer_handles::PeerHandles;
use crate::rollback_config::auto_input_delay;
use crate::room_api_types::SeatRole;
use crate::sync_transfer::outgoing_sync;
use crate::PeerKey;

/// Resync rebuilds with the same handles, so it needs every player of the game.
const RESYNC_PLAYER_GONE: &str = "a player left the game: resync is not possible";

impl LobbyState {
    pub(super) fn on_host_event(
        &mut self,
        event: LobbyEvent,
        now: Instant,
        out: &mut Vec<LobbyEffect>,
    ) {
        match event {
            LobbyEvent::WarmupDone => self.warmup_done = true,
            LobbyEvent::StartPressed if self.can_start() => self.begin_sync(out),
            LobbyEvent::SyncData(payload) if self.phase == LobbyPhase::Syncing => {
                let warmup = self.cfg.fingerprint.warmup_frames;
                match outgoing_sync(&payload, warmup) {
                    Ok(messages) => {
                        for msg in messages {
                            self.send(self.guests(), msg, out);
                        }
                        self.check_acks(out); // no guest left: nothing to wait for
                    }
                    Err(err) => self.end(EndReason::Error(err.to_string()), out),
                }
            }
            LobbyEvent::ResyncRequested if self.phase == LobbyPhase::Running => {
                if self.session_player_gone() {
                    self.notice = Some(RESYNC_PLAYER_GONE.into());
                    return out.push(LobbyEffect::NotifyUi);
                }
                self.epoch = self.epoch.wrapping_add(1);
                self.phase = LobbyPhase::Resyncing;
                self.acks.clear();
                self.resync_deadline = Some(now + RESYNC_TIMEOUT);
                self.send(self.guests(), LobbyMessage::Resync, out);
                out.push(LobbyEffect::DropDriver);
                out.push(LobbyEffect::NotifyUi);
                self.check_acks(out);
            }
            _ => {}
        }
    }

    /// `Syncing`: every guest gets the same `SyncBegin … SyncDone` (after `SyncData`).
    fn begin_sync(&mut self, out: &mut Vec<LobbyEffect>) {
        self.phase = LobbyPhase::Syncing;
        self.acks.clear();
        self.resync_deadline = None;
        out.push(LobbyEffect::PrepareSync);
        out.push(LobbyEffect::NotifyUi);
    }

    /// Advances once every guest acked (`SyncAck` or `ResyncReady`).
    pub(super) fn check_acks(&mut self, out: &mut Vec<LobbyEffect>) {
        let resyncing = matches!(self.phase, LobbyPhase::Resyncing | LobbyPhase::Syncing);
        if resyncing && self.session_player_gone() {
            // Drivers are dropped already: end for everyone instead of shifting handles.
            self.send(self.guests(), LobbyMessage::Leave, out);
            return self.end(EndReason::Error(RESYNC_PLAYER_GONE.into()), out);
        }
        if !self.guests().iter().all(|p| self.acks.contains(p)) {
            return;
        }
        match self.phase {
            LobbyPhase::Resyncing => self.begin_sync(out),
            LobbyPhase::Syncing if self.guests().is_empty() && !self.driver_started => {
                self.phase = LobbyPhase::InLobby; // everyone left before the first Start
                out.push(LobbyEffect::NotifyUi);
            }
            LobbyPhase::Syncing => self.send_start(out),
            _ => {}
        }
    }

    fn send_start(&mut self, out: &mut Vec<LobbyEffect>) {
        let me = self.me.unwrap_or(PeerKey(0));
        let pick = |player: bool| -> Vec<PeerKey> {
            let members = self.roster.iter().filter(|m| m.role.is_player() == player);
            members.map(|m| m.peer).collect()
        };
        // Resync keeps the running session's player handles (= libretro ports, F23).
        let players = match &self.session_handles {
            Some(prev) => prev.players().iter().map(|p| p.0).collect(),
            None => pick(true),
        };
        let handles = PeerHandles::assign(me, &players, &pick(false));
        let worst_rtt = self
            .roster
            .iter()
            .filter(|m| m.role.is_player())
            .filter_map(|m| m.rtt_ms)
            .max();
        let fps_milli = self.cfg.fingerprint.fps_milli;
        let input_delay = self
            .cfg
            .input_delay
            .unwrap_or_else(|| auto_input_delay(worst_rtt.unwrap_or(0), fps_milli));
        let start = LobbyMessage::Start {
            input_delay,
            epoch: self.epoch,
            players: handles.players().to_vec(),
            spectators: handles.spectators().to_vec(),
        };
        self.send(self.guests(), start, out);
        self.session_handles = Some(handles.clone());
        let plan = StartPlan {
            handles,
            me,
            role: SeatRole::Host,
            input_delay,
            epoch: self.epoch,
        };
        out.push(match self.driver_started {
            true => LobbyEffect::RebuildDriver(plan),
            false => LobbyEffect::StartDriver(plan),
        });
        self.driver_started = true;
        self.phase = LobbyPhase::Running;
        out.push(LobbyEffect::NotifyUi);
    }

    /// Host: a player of the running ggrs session is no longer in the roster.
    fn session_player_gone(&self) -> bool {
        let Some(prev) = &self.session_handles else {
            return false;
        };
        let present = |p: &PeerKey| self.roster.iter().any(|m| m.peer == *p);
        !prev.players().iter().all(|(p, _)| present(p))
    }
}
