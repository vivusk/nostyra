//! Lobby side of a redraw: socket peers, lobby messages and timers in, effects out
//! (messages, sync on the core, driver start / rebuild / drop), then the launcher
//! events and the end of the session.

use std::time::{Duration, Instant};

use ipc_protocol::NetplayEvent;
use netplay_session::lobby_state::{EndReason, LobbyEffect, LobbyEvent, LobbyPhase, StartPlan};
use netplay_session::rollback_config::RollbackConfig;
use netplay_session::rollback_driver::RollbackDriver;
use netplay_session::socket_thread::PeerChange;

use super::netplay_core::NetplayCore;
use super::netplay_exit::end_outcome;
use super::NetplayRuntime;

/// How long the end screen stays before the runner closes.
const HOST_LEFT_DELAY: Duration = Duration::from_secs(3);
const REFUSED_DELAY: Duration = Duration::from_secs(5);

impl NetplayRuntime {
    /// Once per redraw: peers, lobby messages, timers → effects (sync on `core`).
    pub(crate) fn poll_lobby(&mut self, core: &mut dyn NetplayCore, now: Instant) {
        if self.exit_at.is_some() {
            return;
        }
        // `events` keeps its capacity across redraws: no allocation once warm.
        let mut events = std::mem::take(&mut self.events);
        events.push_back(LobbyEvent::Tick);
        if !self.warmup_reported {
            self.warmup_reported = true; // the warm-up ran at load (fingerprint)
            events.push_back(LobbyEvent::WarmupDone);
        }
        if let Some(me) = self.socket.local_peer() {
            events.push_back(LobbyEvent::Connected { me });
        }
        match self.socket.poll_peers() {
            Ok(changes) => events.extend(changes.into_iter().map(|change| match change {
                PeerChange::Joined(peer) => LobbyEvent::PeerJoined(peer),
                PeerChange::Left(peer) => LobbyEvent::PeerLeft(peer),
            })),
            Err(err) => self.fail(err.to_string(), now),
        }
        let received = self.link.receive();
        events.extend(
            received
                .into_iter()
                .map(|(p, m)| LobbyEvent::received(p, m)),
        );
        if self.autostart_due() {
            events.push_back(LobbyEvent::StartPressed);
        }
        self.events = events;
        self.apply_queued(core, now);
    }

    /// Host pressed Start (Enter / pad Start in the waiting room).
    pub(crate) fn press_start(&mut self, core: &mut dyn NetplayCore, now: Instant) {
        if self.lobby.can_start() {
            self.events.push_back(LobbyEvent::StartPressed);
            self.apply_queued(core, now);
        }
    }

    /// Host pressed Resync (menu row) after a desync warning.
    pub(crate) fn request_resync(&mut self, core: &mut dyn NetplayCore, now: Instant) {
        self.events.push_back(LobbyEvent::ResyncRequested);
        self.apply_queued(core, now);
    }

    fn autostart_due(&self) -> bool {
        let Some(spectators) = self.options.autostart else {
            return false;
        };
        let roster = self.lobby.roster();
        let players = roster.iter().filter(|m| m.role.is_player()).count();
        let watchers = roster.len() - players;
        self.lobby.can_start()
            && players >= usize::from(self.options.fingerprint.max_players)
            && watchers >= usize::from(spectators)
    }

    /// Applies the queued events (and the events their effects produce).
    fn apply_queued(&mut self, core: &mut dyn NetplayCore, now: Instant) {
        let mut queue = std::mem::take(&mut self.events);
        while let Some(event) = queue.pop_front() {
            for effect in self.lobby.apply(event, now) {
                queue.extend(self.perform(effect, core, now));
            }
        }
        self.events = queue; // empty, capacity kept
        self.after_lobby(now);
    }

    fn perform(
        &mut self,
        effect: LobbyEffect,
        core: &mut dyn NetplayCore,
        now: Instant,
    ) -> Option<LobbyEvent> {
        match effect {
            LobbyEffect::Send { to, msg } => {
                if let Err(err) = self.link.send(&to, &msg) {
                    tracing::warn!("netplay lobby send failed: {err}");
                }
            }
            LobbyEffect::PrepareSync => match core.sync_payload() {
                Ok(payload) => return Some(LobbyEvent::SyncData(payload)),
                Err(err) => self.fail(err, now),
            },
            // The warm-up ran right after load (its count is in the fingerprint).
            LobbyEffect::RunWarmup(_) => {}
            LobbyEffect::LoadSync(payload) => match core.load_sync(&payload) {
                Ok(()) => return Some(LobbyEvent::SyncLoaded),
                Err(err) => self.fail(err, now),
            },
            LobbyEffect::StartDriver(plan) | LobbyEffect::RebuildDriver(plan) => {
                self.start_driver(plan, now);
            }
            LobbyEffect::DropDriver => {
                self.driver = None;
                self.ram_crc.clear();
                self.touch();
            }
            LobbyEffect::NotifyUi => self.touch(),
        }
        None
    }

    fn start_driver(&mut self, plan: StartPlan, now: Instant) {
        let config = RollbackConfig::new(plan.clone(), self.options.fingerprint.fps_milli);
        match RollbackDriver::new(&config, self.socket.ggrs_channel()) {
            Ok(driver) => {
                let players = plan.handles.num_players() as u8;
                tracing::info!(players, input_delay = plan.input_delay, "netplay started");
                self.outbox.push(NetplayEvent::Started {
                    players,
                    input_delay: plan.input_delay.min(255) as u8,
                });
                self.driver = Some(driver);
                self.handles = Some(plan.handles);
                self.ram_crc.clear();
                self.pacer.reset();
                self.stats.reset(now);
                self.touch();
            }
            Err(err) => self.fail(err.to_string(), now),
        }
    }

    /// Launcher events for what changed, then the end of the session.
    pub(super) fn after_lobby(&mut self, now: Instant) {
        for member in self.lobby.roster() {
            if self.nicks.get(&member.peer) != Some(&member.nickname) {
                self.nicks.insert(member.peer, member.nickname.clone());
            }
        }
        self.emit_lobby_changes();
        let ended = match (&self.local_end, self.lobby.phase()) {
            (Some(reason), _) => Some(reason.clone()),
            (None, LobbyPhase::Ended(reason)) => Some(reason.clone()),
            _ => None,
        };
        let Some(reason) = ended.filter(|_| self.exit_at.is_none()) else {
            return;
        };
        let (launcher_reason, exit) = end_outcome(&reason);
        let delay = match &reason {
            EndReason::HostLeft => HOST_LEFT_DELAY,
            EndReason::Refused(_) => REFUSED_DELAY,
            EndReason::Left | EndReason::Error(_) => Duration::ZERO,
        };
        tracing::info!(?launcher_reason, "netplay session ended");
        self.outbox.push(NetplayEvent::Ended {
            reason: launcher_reason,
        });
        self.driver = None;
        self.end_reason = Some(reason);
        self.exit_at = Some((now + delay, exit));
        self.touch();
    }
}
