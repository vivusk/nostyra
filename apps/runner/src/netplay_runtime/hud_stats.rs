//! Running-session numbers the HUD shows and the warnings it keeps (stall, desync,
//! a peer that left), reset with every new ggrs session.

use std::time::{Duration, Instant};

use ipc_protocol::NetplayRosterEntry;

/// Stats event cadence.
pub(super) const STATS_EVERY: Duration = Duration::from_secs(2);

/// Running-session numbers for the HUD.
#[derive(Debug)]
pub(crate) struct HudStats {
    pub(super) next_stats: Instant,
    pub ping_ms: Option<u32>,
    pub rollback_fps: u32,
    pub relayed: bool,
    pub(super) stall_since: Option<Instant>,
    pub stalled: bool,
    pub desync_frame: Option<i32>,
    pub(super) peer_left: Option<(String, Instant)>,
    pub(super) last_roster: Vec<NetplayRosterEntry>,
    pub(super) sync_percent: Option<u8>,
}

impl HudStats {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            next_stats: now + STATS_EVERY,
            ping_ms: None,
            rollback_fps: 0,
            relayed: false,
            stall_since: None,
            stalled: false,
            desync_frame: None,
            peer_left: None,
            last_roster: Vec::new(),
            sync_percent: None,
        }
    }

    /// A (re)started ggrs session: warnings and numbers start over.
    pub(crate) fn reset(&mut self, now: Instant) {
        let roster = std::mem::take(&mut self.last_roster);
        *self = Self::new(now);
        self.last_roster = roster;
    }

    pub(crate) fn peer_left(&mut self, nick: &str) {
        self.peer_left = Some((nick.to_owned(), Instant::now()));
    }
}
