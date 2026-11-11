//! Lobby state machine of one peer (host, player or spectator), no I/O:
//! `apply(event, now)` returns the effects the runner performs in order.
//! `Connecting → Handshaking → InLobby → Syncing → Starting → Running → Ended`,
//! plus `Resyncing` (F23). Host rules: `host_rules.rs`; guest rules: `guest_rules.rs`.

mod common_rules;
mod guest_rules;
mod guest_sync_rules;
mod host_rules;
mod host_sync_rules;
mod lobby_events;

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

pub use lobby_events::{EndReason, LobbyConfig, LobbyEffect, LobbyEvent, LobbyPhase, StartPlan};

use crate::lobby_protocol::LobbyMember;
use crate::peer_handles::PeerHandles;
use crate::room_api_types::SeatRole;
use crate::sync_transfer::SyncInbox;
use crate::PeerKey;

/// Spectators per room (server enforces the same cap).
pub const MAX_SPECTATORS: usize = 8;
/// Host waits this long for every `ResyncReady`.
pub const RESYNC_TIMEOUT: Duration = Duration::from_secs(5);
/// Lobby ping cadence (host → guests).
pub const PING_INTERVAL: Duration = Duration::from_secs(1);

/// One peer's lobby.
pub struct LobbyState {
    cfg: LobbyConfig,
    phase: LobbyPhase,
    me: Option<PeerKey>,
    host: Option<PeerKey>,
    /// Host: authoritative, join order, own line first. Guest: last copy from the host.
    roster: Vec<LobbyMember>,
    my_handle: Option<usize>,
    /// Peers the socket reports connected (host: to detect un-greeted peers).
    connected: BTreeSet<PeerKey>,
    refused: BTreeSet<PeerKey>,
    warmup_done: bool,
    /// Host: `SyncAck` / `ResyncReady` received.
    acks: BTreeSet<PeerKey>,
    inbox: Option<SyncInbox>,
    sync_progress: f32,
    epoch: u8,
    driver_started: bool,
    /// Host: handles of the running ggrs session (a resync keeps them, F23).
    session_handles: Option<PeerHandles>,
    /// Last thing to tell this peer's user (refused joiner, resync not possible).
    notice: Option<String>,
    resync_deadline: Option<Instant>,
    next_ping: Option<Instant>,
    pings: HashMap<u64, (PeerKey, Instant)>,
    nonce: u64,
}

impl LobbyState {
    /// Lobby for `cfg.role` (host or guest).
    pub fn new(cfg: LobbyConfig) -> Self {
        Self {
            cfg,
            phase: LobbyPhase::Connecting,
            me: None,
            host: None,
            roster: Vec::new(),
            my_handle: None,
            connected: BTreeSet::new(),
            refused: BTreeSet::new(),
            warmup_done: false,
            acks: BTreeSet::new(),
            inbox: None,
            sync_progress: 0.0,
            epoch: 0,
            driver_started: false,
            session_handles: None,
            notice: None,
            resync_deadline: None,
            next_ping: None,
            pings: HashMap::new(),
            nonce: 0,
        }
    }

    pub fn phase(&self) -> &LobbyPhase {
        &self.phase
    }

    pub fn roster(&self) -> &[LobbyMember] {
        &self.roster
    }

    /// The host's `PeerKey` (guests: the `Welcome` sender, F31).
    pub fn host(&self) -> Option<PeerKey> {
        self.host
    }

    pub fn my_handle(&self) -> Option<usize> {
        self.my_handle
    }

    /// Host: last notice for the UI (a peer refused for undecodable messages,
    /// a resync refused because a player left).
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Guest transfer progress 0.0 ..= 1.0.
    pub fn sync_progress(&self) -> f32 {
        self.sync_progress
    }

    pub fn is_host(&self) -> bool {
        self.cfg.role == SeatRole::Host
    }

    /// Host: Start would be accepted now (warm-up done, every connected peer
    /// greeted, at least one guest).
    pub fn can_start(&self) -> bool {
        let greeted =
            |p: &PeerKey| self.refused.contains(p) || self.roster.iter().any(|m| m.peer == *p);
        self.is_host()
            && self.phase == LobbyPhase::InLobby
            && self.warmup_done
            && self.roster.len() > 1
            && self.connected.iter().all(greeted)
    }

    /// Feeds one event; returns the effects to perform in order.
    pub fn apply(&mut self, event: LobbyEvent, now: Instant) -> Vec<LobbyEffect> {
        if matches!(self.phase, LobbyPhase::Ended(_)) {
            return Vec::new();
        }
        let mut out = Vec::new();
        match event {
            LobbyEvent::Connected { me } => self.on_connected(me, &mut out),
            LobbyEvent::PeerJoined(peer) => self.on_peer_joined(peer, &mut out),
            LobbyEvent::PeerLeft(peer) => self.on_peer_left(peer, &mut out),
            LobbyEvent::Message(peer, msg) => self.on_message(peer, msg, now, &mut out),
            LobbyEvent::BadMessage(peer, err) => self.on_bad_message(peer, err, &mut out),
            LobbyEvent::Quit => self.on_quit(&mut out),
            LobbyEvent::Tick => self.on_tick(now, &mut out),
            event if self.is_host() => self.on_host_event(event, now, &mut out),
            LobbyEvent::SyncLoaded => self.on_sync_loaded(&mut out),
            _ => {}
        }
        out
    }
}

#[cfg(test)]
#[path = "lobby_state_harness_tests.rs"]
mod harness;

#[cfg(test)]
#[path = "lobby_state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "lobby_state_join_tests.rs"]
mod join_tests;

#[cfg(test)]
#[path = "lobby_state_resync_tests.rs"]
mod resync_tests;
