//! Lobby test harness: an in-memory room that delivers `Send` effects and
//! answers `PrepareSync` / `LoadSync` like the runner would.

#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::*;
use crate::lobby_protocol::RefusalReason;
use crate::session_fingerprint::SessionFingerprint;
use crate::sync_transfer::SyncPayload;

pub(super) const HOST: PeerKey = PeerKey(100);

/// In-memory room: delivers `Send` effects, answers `PrepareSync` / `LoadSync`.
pub(super) struct Room {
    pub nodes: Vec<(PeerKey, LobbyState, Vec<LobbyEffect>)>,
    pub now: Instant,
    pub muted: Vec<PeerKey>,
    queue: VecDeque<(PeerKey, LobbyEvent)>,
}

pub(super) fn cfg(role: SeatRole, fp: SessionFingerprint) -> LobbyConfig {
    LobbyConfig {
        role,
        nickname: format!("{role:?}"),
        fingerprint: fp,
        sram_len: 4,
        rtc_len: None,
        input_delay: None,
    }
}

pub(super) fn payload() -> SyncPayload {
    SyncPayload {
        sram: vec![1, 2, 3, 4],
        rtc: None,
        state: vec![9; 40_000],
    }
}

impl Room {
    pub fn new(fp: SessionFingerprint) -> Self {
        let mut room = Self {
            nodes: Vec::new(),
            now: Instant::now(),
            muted: Vec::new(),
            queue: VecDeque::new(),
        };
        room.add(HOST, cfg(SeatRole::Host, fp));
        room.event(HOST, LobbyEvent::WarmupDone);
        room
    }

    /// Connects `key`; every present peer and `key` see each other join.
    pub fn add(&mut self, key: PeerKey, cfg: LobbyConfig) {
        let others: Vec<PeerKey> = self.nodes.iter().map(|n| n.0).collect();
        self.nodes.push((key, LobbyState::new(cfg), Vec::new()));
        self.queue
            .push_back((key, LobbyEvent::Connected { me: key }));
        for other in others {
            self.queue.push_back((other, LobbyEvent::PeerJoined(key)));
            self.queue.push_back((key, LobbyEvent::PeerJoined(other)));
        }
        self.pump();
    }

    pub fn leave(&mut self, key: PeerKey) {
        self.nodes.retain(|n| n.0 != key);
        let others: Vec<PeerKey> = self.nodes.iter().map(|n| n.0).collect();
        others
            .into_iter()
            .for_each(|o| self.queue.push_back((o, LobbyEvent::PeerLeft(key))));
        self.pump();
    }

    pub fn event(&mut self, key: PeerKey, event: LobbyEvent) {
        self.queue.push_back((key, event));
        self.pump();
    }

    fn pump(&mut self) {
        while let Some((key, event)) = self.queue.pop_front() {
            let now = self.now;
            let Some(node) = self.nodes.iter_mut().find(|n| n.0 == key) else {
                continue;
            };
            let effects = node.1.apply(event, now);
            for effect in &effects {
                match effect {
                    LobbyEffect::Send { to, msg } if !self.muted.contains(&key) => {
                        to.iter().for_each(|&t| {
                            self.queue
                                .push_back((t, LobbyEvent::Message(key, msg.clone())))
                        });
                    }
                    LobbyEffect::PrepareSync => {
                        self.queue.push_back((key, LobbyEvent::SyncData(payload())))
                    }
                    LobbyEffect::LoadSync(_) => self.queue.push_back((key, LobbyEvent::SyncLoaded)),
                    _ => {}
                }
            }
            node.2.extend(effects);
        }
    }

    pub fn node(&self, key: PeerKey) -> &(PeerKey, LobbyState, Vec<LobbyEffect>) {
        self.nodes.iter().find(|n| n.0 == key).unwrap()
    }

    pub fn phase(&self, key: PeerKey) -> LobbyPhase {
        self.node(key).1.phase().clone()
    }

    /// Effects of `key` since the last call.
    pub fn take(&mut self, key: PeerKey) -> Vec<LobbyEffect> {
        let node = self.nodes.iter_mut().find(|n| n.0 == key).unwrap();
        std::mem::take(&mut node.2)
    }

    pub fn plans(&mut self, key: PeerKey) -> Vec<LobbyEffect> {
        let effects = self.take(key);
        effects
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    LobbyEffect::StartDriver(_) | LobbyEffect::RebuildDriver(_)
                )
            })
            .collect()
    }

    pub fn start(&mut self) {
        self.event(HOST, LobbyEvent::StartPressed);
    }

    pub fn later(&mut self, by: Duration) {
        self.now += by;
    }
}

pub(super) fn refusal(room: &Room, key: PeerKey) -> RefusalReason {
    match room.phase(key) {
        LobbyPhase::Ended(EndReason::Refused(reason)) => reason,
        other => panic!("not refused: {other:?}"),
    }
}
