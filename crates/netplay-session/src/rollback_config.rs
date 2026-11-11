//! ggrs session setup: the input-delay rule (pure: the host computes it in the
//! lobby) and, with `native`, the `ggrs::Config` plus the `SessionBuilder` for the
//! local peer's role (host / player P2P, spectator → host only).

use std::time::Duration;

/// Rollback window (ggrs default).
pub const MAX_PREDICTION_WINDOW: usize = 8;
/// Frames between desync checksum comparisons.
pub const DESYNC_INTERVAL: u32 = 120;
pub const DISCONNECT_TIMEOUT: Duration = Duration::from_millis(2000);
pub const DISCONNECT_NOTIFY_DELAY: Duration = Duration::from_millis(500);
/// Spectator falls back to catch-up speed beyond this.
pub const MAX_FRAMES_BEHIND: usize = 10;
/// Largest auto input delay (frames).
pub const MAX_AUTO_INPUT_DELAY: usize = 4;

/// `clamp(round(ping / 2 / frame_ms), 1, 4)`: half the round trip hidden by delay.
pub fn auto_input_delay(ping_ms: u32, fps_milli: u32) -> usize {
    let frame_ms = 1_000_000.0 / f64::from(fps_milli.max(1));
    let frames = (f64::from(ping_ms) / 2.0 / frame_ms).round();
    (frames as usize).clamp(1, MAX_AUTO_INPUT_DELAY)
}

#[cfg(feature = "native")]
pub use native::{NetplayConfig, RollbackConfig};

#[cfg(feature = "native")]
mod native {
    use ggrs::{DesyncDetection, PlayerType, SessionBuilder};

    use super::*;
    use crate::lobby_state::StartPlan;
    use crate::netplay_input::{InputPredictor, NetplayInput};
    use crate::rollback_driver::DriverSession;
    use crate::room_api_types::SeatRole;
    use crate::shared_ggrs_channel::SharedGgrsChannel;
    use crate::{NetplayError, PeerKey};

    /// ggrs parameters: wire input, repeat-last prediction, state = the saved frame
    /// (the bytes live in the driver's reusable ring), address = [`PeerKey`].
    pub struct NetplayConfig;

    impl ggrs::Config for NetplayConfig {
        type Input = NetplayInput;
        type InputPredictor = InputPredictor;
        type State = ggrs::Frame;
        type Address = PeerKey;
    }

    /// Everything one ggrs session is built from.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RollbackConfig {
        pub plan: StartPlan,
        /// From the fingerprint (`av_info` fps × 1000).
        pub fps_milli: u32,
        pub desync_interval: u32,
        pub max_prediction_window: usize,
    }

    impl RollbackConfig {
        /// Plan defaults (window 8, desync every 120 frames).
        pub fn new(plan: StartPlan, fps_milli: u32) -> Self {
            Self {
                plan,
                fps_milli,
                desync_interval: DESYNC_INTERVAL,
                max_prediction_window: MAX_PREDICTION_WINDOW,
            }
        }

        fn builder(&self) -> Result<SessionBuilder<NetplayConfig>, NetplayError> {
            let fps = ((self.fps_milli as usize + 500) / 1000).max(1);
            Ok(SessionBuilder::<NetplayConfig>::new()
                .with_num_players(self.plan.handles.num_players())?
                .with_fps(fps)?
                .with_input_delay(self.plan.input_delay)
                .with_max_prediction_window(self.max_prediction_window)
                .with_desync_detection_mode(DesyncDetection::On {
                    interval: self.desync_interval,
                })
                .with_disconnect_timeout(DISCONNECT_TIMEOUT)
                .with_disconnect_notify_delay(DISCONNECT_NOTIFY_DELAY)
                .with_max_frames_behind(MAX_FRAMES_BEHIND)?)
        }

        /// Builds the ggrs session of this peer's role on `socket`.
        pub(crate) fn start(
            &self,
            socket: SharedGgrsChannel,
        ) -> Result<DriverSession, NetplayError> {
            let plan = &self.plan;
            let mut builder = self.builder()?;
            if plan.role == SeatRole::Spectator {
                let host = plan.handles.host();
                let session = builder.start_spectator_session(host, socket);
                return Ok(DriverSession::Spectator(Box::new(session)));
            }
            for &(peer, handle) in plan.handles.players() {
                let kind = if peer == plan.me {
                    PlayerType::Local
                } else {
                    PlayerType::Remote(peer)
                };
                builder = builder.add_player(kind, handle)?;
            }
            // Spectators talk to the host only (ggrs relays inputs through it).
            if plan.local_handle() == Some(0) {
                for &(peer, handle) in plan.handles.spectators() {
                    builder = builder.add_player(PlayerType::Spectator(peer), handle)?;
                }
            }
            Ok(DriverSession::P2P(Box::new(
                builder.start_p2p_session(socket)?,
            )))
        }
    }

    impl From<ggrs::GgrsError> for NetplayError {
        fn from(err: ggrs::GgrsError) -> Self {
            NetplayError::Rollback(err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_delay_auto_table() {
        let rows = [
            (60_000, [(20, 1), (80, 2), (140, 4), (300, 4)]),
            (50_000, [(20, 1), (80, 2), (140, 4), (300, 4)]),
        ];
        for (fps, cases) in rows {
            for (ping, delay) in cases {
                assert_eq!(auto_input_delay(ping, fps), delay, "{ping} ms at {fps}");
            }
        }
        assert_eq!(auto_input_delay(0, 60_000), 1, "never below 1");
        assert_eq!(auto_input_delay(100, 50_000), 3, "100 / 2 / 20 = 2.5 → 3");
        assert_eq!(auto_input_delay(100, 0), 1, "fps 0 guarded");
    }
}
