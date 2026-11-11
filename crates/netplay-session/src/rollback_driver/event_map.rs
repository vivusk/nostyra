//! ggrs events → [`NetplayEvent`] (addresses stay [`PeerKey`]s; no ids in text).

use ggrs::{Frame, GgrsEvent};

use super::DriverSession;
use crate::rollback_config::NetplayConfig;
use crate::PeerKey;

/// What the session reports to the HUD / IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetplayEvent {
    Synchronizing {
        peer: PeerKey,
        count: u32,
        total: u32,
    },
    Synchronized {
        peer: PeerKey,
    },
    Disconnected {
        peer: PeerKey,
    },
    NetworkInterrupted {
        peer: PeerKey,
        disconnect_timeout_ms: u64,
    },
    NetworkResumed {
        peer: PeerKey,
    },
    /// ggrs suggests waiting; the driver already slows the pacer instead.
    WaitRecommendation {
        skip_frames: u32,
    },
    /// Checksums of `frame` differ with `peer`: HUD warning, host may Resync.
    DesyncDetected {
        frame: Frame,
        peer: PeerKey,
    },
}

fn map(event: GgrsEvent<NetplayConfig>) -> NetplayEvent {
    match event {
        GgrsEvent::Synchronizing { addr, total, count } => NetplayEvent::Synchronizing {
            peer: addr,
            count,
            total,
        },
        GgrsEvent::Synchronized { addr } => NetplayEvent::Synchronized { peer: addr },
        GgrsEvent::Disconnected { addr } => NetplayEvent::Disconnected { peer: addr },
        GgrsEvent::NetworkInterrupted {
            addr,
            disconnect_timeout,
        } => NetplayEvent::NetworkInterrupted {
            peer: addr,
            disconnect_timeout_ms: u64::try_from(disconnect_timeout).unwrap_or(u64::MAX),
        },
        GgrsEvent::NetworkResumed { addr } => NetplayEvent::NetworkResumed { peer: addr },
        GgrsEvent::WaitRecommendation { skip_frames } => {
            NetplayEvent::WaitRecommendation { skip_frames }
        }
        GgrsEvent::DesyncDetected { frame, addr, .. } => {
            NetplayEvent::DesyncDetected { frame, peer: addr }
        }
    }
}

/// Moves the session's pending events into `out`; true when one was a desync.
pub(super) fn drain_events(session: &mut DriverSession, out: &mut Vec<NetplayEvent>) -> bool {
    let before = out.len();
    match session {
        DriverSession::P2P(s) => out.extend(s.events().map(map)),
        DriverSession::Spectator(s) => out.extend(s.events().map(map)),
    }
    out[before..]
        .iter()
        .any(|e| matches!(e, NetplayEvent::DesyncDetected { .. }))
}
