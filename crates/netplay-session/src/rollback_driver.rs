//! `RollbackDriver`: one ggrs session (P2P for host / players, spectator session
//! for watchers) on the shared ggrs channel. The core thread calls [`tick`] once per
//! emulated frame; requests become [`RollbackHost`] calls (`request_apply.rs`),
//! events become [`NetplayEvent`]s (`event_map.rs`). [`rebuild`] starts the next
//! session on the same channel after a resync (F23).
//!
//! [`tick`]: RollbackDriver::tick
//! [`rebuild`]: RollbackDriver::rebuild

mod driver_queries;
mod event_map;
mod request_apply;

use std::time::Instant;

use ggrs::{Frame, GgrsError, InputStatus, P2PSession, SessionState, SpectatorSession};

pub use event_map::NetplayEvent;
use request_apply::{apply_requests, FrameCursor, StateRing};

use crate::net_stats::RollbackMeter;
use crate::netplay_input::NetplayInput;
use crate::peer_channel::SharedPeerChannel;
use crate::rollback_config::{NetplayConfig, RollbackConfig};
use crate::shared_ggrs_channel::SharedGgrsChannel;
use crate::NetplayError;

/// Frames ahead of the slowest peer before the pacer slows down by 10 %.
pub const SLOWDOWN_FRAMES_AHEAD: i32 = 2;
/// Undrained events kept (oldest dropped beyond).
const MAX_PENDING_EVENTS: usize = 256;

/// The runner's side of the rollback loop (core thread, `!Send` core).
pub trait RollbackHost {
    /// Serializes the state at the start of `frame` into `buf` (empty, reused) and
    /// returns its desync checksum (`desync_checksum` over RAM + SRAM).
    fn save(&mut self, frame: Frame, buf: &mut Vec<u8>) -> Result<u128, NetplayError>;
    /// Restores the state saved for `frame`.
    fn load(&mut self, frame: Frame, state: &[u8]) -> Result<(), NetplayError>;
    /// Runs `frame` with one input per player. `is_new` = first advance of this frame
    /// (achievement / save hooks run only then, F20); `last_of_tick` = present video.
    fn advance(
        &mut self,
        frame: Frame,
        inputs: &[(NetplayInput, InputStatus)],
        is_new: bool,
        last_of_tick: bool,
    );
}

/// Result of one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickOutcome {
    /// ggrs still synchronizing with the peers.
    Waiting,
    /// Too far ahead (`PredictionThreshold`) or no host input yet (spectator).
    Stalled,
    Advanced {
        new_frames: u32,
        resim_frames: u32,
    },
}

pub(crate) enum DriverSession {
    P2P(Box<P2PSession<NetplayConfig>>),
    Spectator(Box<SpectatorSession<NetplayConfig>>),
}

/// One ggrs session plus its bookkeeping.
pub struct RollbackDriver {
    session: DriverSession,
    local_handles: Vec<usize>,
    cursor: FrameCursor,
    ring: StateRing,
    events: Vec<NetplayEvent>,
    meter: RollbackMeter,
    stall_count: u32,
    desync: bool,
}

impl RollbackDriver {
    /// Starts the session `config` describes on `channel` (epoch from the plan).
    pub fn new(config: &RollbackConfig, channel: SharedPeerChannel) -> Result<Self, NetplayError> {
        let session = config.start(SharedGgrsChannel::new(channel, config.plan.epoch))?;
        Ok(Self {
            session,
            local_handles: config.plan.local_handle().into_iter().collect(),
            cursor: FrameCursor::START,
            ring: StateRing::new(config.max_prediction_window + 2),
            events: Vec::new(),
            meter: RollbackMeter::new(Instant::now()),
            stall_count: 0,
            desync: false,
        })
    }

    /// Drops the current session and starts the next one on the same channel; frame
    /// counters, ring and stats start over (the buffers are kept).
    pub fn rebuild(
        &mut self,
        config: &RollbackConfig,
        channel: SharedPeerChannel,
    ) -> Result<(), NetplayError> {
        self.session = config.start(SharedGgrsChannel::new(channel, config.plan.epoch))?;
        self.local_handles = config.plan.local_handle().into_iter().collect();
        self.cursor = FrameCursor::START;
        self.ring.reset(config.max_prediction_window + 2);
        self.events.clear();
        self.meter = RollbackMeter::new(Instant::now());
        (self.stall_count, self.desync) = (0, false);
        Ok(())
    }

    /// One emulated frame: poll, add `local` (one input per local handle), advance.
    /// `last_due` = this is the last due frame of the redraw (F35).
    pub fn tick(
        &mut self,
        local: &[NetplayInput],
        host: &mut dyn RollbackHost,
        last_due: bool,
    ) -> Result<TickOutcome, NetplayError> {
        let result = match &mut self.session {
            DriverSession::P2P(session) => {
                session.poll_remote_clients();
                if session.current_state() != SessionState::Running {
                    Ok(None)
                } else {
                    for (i, &handle) in self.local_handles.iter().enumerate() {
                        session
                            .add_local_input(handle, local.get(i).copied().unwrap_or_default())?;
                    }
                    session.advance_frame().map(Some)
                }
            }
            DriverSession::Spectator(session) => {
                session.poll_remote_clients();
                match session.current_state() {
                    SessionState::Running => session.advance_frame().map(Some),
                    SessionState::Synchronizing => Ok(None),
                }
            }
        };
        self.collect_events();
        let outcome = match result {
            Ok(None) => TickOutcome::Waiting,
            Ok(Some(requests)) => {
                let applied =
                    apply_requests(requests, &mut self.cursor, &mut self.ring, host, last_due)?;
                self.meter.record(applied.resim_frames, Instant::now());
                if applied.new_frames == 0 {
                    // ggrs 0.13 P2P: at the prediction threshold `advance_frame` is
                    // `Ok` without an `AdvanceFrame` (only the spectator returns Err).
                    self.stall_count = self.stall_count.saturating_add(1);
                    TickOutcome::Stalled
                } else {
                    TickOutcome::Advanced {
                        new_frames: applied.new_frames,
                        resim_frames: applied.resim_frames,
                    }
                }
            }
            Err(GgrsError::PredictionThreshold) => {
                self.stall_count = self.stall_count.saturating_add(1);
                TickOutcome::Stalled
            }
            Err(err) => return Err(err.into()),
        };
        Ok(outcome)
    }

    fn collect_events(&mut self) {
        self.desync |= event_map::drain_events(&mut self.session, &mut self.events);
        if self.events.len() > MAX_PENDING_EVENTS {
            let extra = self.events.len() - MAX_PENDING_EVENTS;
            self.events.drain(..extra);
        }
    }

    /// Events since the last call.
    pub fn drain_events(&mut self) -> Vec<NetplayEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
#[path = "rollback_driver_harness_tests.rs"]
mod harness;

#[cfg(test)]
#[path = "rollback_driver_tests.rs"]
mod tests;
