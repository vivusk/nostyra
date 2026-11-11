//! Read-only views of the driver: frame counters, sync state, pacer slowdown,
//! ring counters and the HUD / IPC stats snapshot.

use std::time::Instant;

use ggrs::{Frame, GgrsError, SessionState};

use super::{DriverSession, RollbackDriver, SLOWDOWN_FRAMES_AHEAD};
use crate::net_stats::{NetplayStats, PeerStats};

impl RollbackDriver {
    /// Highest frame every player's input is confirmed for (spectator: last advanced
    /// frame — every spectator frame is confirmed, F34).
    pub fn confirmed_frame(&self) -> Frame {
        match &self.session {
            DriverSession::P2P(session) => session.confirmed_frame(),
            DriverSession::Spectator(_) => self.cursor.head,
        }
    }

    /// Frame the next local input is added for (scripted test input keys on it).
    pub fn current_frame(&self) -> Frame {
        match &self.session {
            DriverSession::P2P(session) => session.current_frame(),
            DriverSession::Spectator(_) => self.cursor.head + 1,
        }
    }

    /// Highest frame ever advanced in this session (`NULL_FRAME` = none yet).
    pub fn head_frame(&self) -> Frame {
        self.cursor.head
    }

    /// True once ggrs synchronized with the peers.
    pub fn is_running(&self) -> bool {
        match &self.session {
            DriverSession::P2P(session) => session.current_state() == SessionState::Running,
            DriverSession::Spectator(session) => session.current_state() == SessionState::Running,
        }
    }

    /// Pacer slowdown asked for: 10 % while ahead of the slowest peer, else 0.
    pub fn slowdown_percent(&self) -> u8 {
        match &self.session {
            DriverSession::P2P(session) if session.frames_ahead() >= SLOWDOWN_FRAMES_AHEAD => 10,
            _ => 0,
        }
    }

    /// `(saves, buffer grows)` of the state ring (steady state: grows stop).
    pub fn ring_counters(&self) -> (u64, u64) {
        (self.ring.saves, self.ring.grows)
    }

    /// HUD / IPC snapshot (`relayed` left `None`: the socket knows it).
    pub fn stats(&mut self) -> NetplayStats {
        let peer = |handle: usize, stats: Result<ggrs::NetworkStats, GgrsError>| {
            let stats = stats.ok();
            PeerStats {
                handle,
                ping_ms: stats.map(|s| u32::try_from(s.ping).unwrap_or(u32::MAX)),
                relayed: None,
                remote_frames_behind: stats.map_or(0, |s| s.remote_frames_behind),
            }
        };
        let peers = match &self.session {
            DriverSession::P2P(session) => {
                let mut handles = session.remote_player_handles();
                handles.extend(session.spectator_handles());
                handles
                    .into_iter()
                    .map(|h| peer(h, session.network_stats(h)))
                    .collect()
            }
            DriverSession::Spectator(session) => vec![peer(0, session.network_stats())],
        };
        NetplayStats {
            peers,
            rollback_frames_last_second: self.meter.last_second(Instant::now()),
            stall_count: self.stall_count,
            desync: self.desync,
            slowdown_percent: self.slowdown_percent(),
        }
    }
}
