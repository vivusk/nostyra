//! Frame side of a redraw once the session runs: the netplay pacer decides how many
//! frames are due (scaled by the ggrs slowdown, cap 12), each one is a driver tick
//! with the local port (or neutral input past the frames limit), then driver
//! events (peer left, desync), stall tracking, stats and the frames-limit check.

use std::time::Instant;

use input_mapping::{netplay_input_from_port, PortState};
use ipc_protocol::NetplayEvent;
use netplay_session::lobby_state::LobbyPhase;
use netplay_session::rollback_driver::{NetplayEvent as DriverEvent, TickOutcome};

use super::netplay_core::NetplayCore;
use super::rollback_host::CoreRollbackHost;
use super::NetplayRuntime;

impl NetplayRuntime {
    /// Runs the frames due now; `local` = this player's port (ignored by spectators).
    /// Returns how many frames were ticked.
    pub(crate) fn run_due_frames(
        &mut self,
        core: &mut dyn NetplayCore,
        local: PortState,
        now: Instant,
    ) -> u32 {
        if self.exit_at.is_some() || *self.lobby.phase() != LobbyPhase::Running {
            return 0;
        }
        let spectator = self.is_spectator();
        let Some(driver) = self.driver.as_mut() else {
            return 0;
        };
        let scale = 1.0 + f64::from(driver.slowdown_percent()) / 100.0;
        self.pacer.set_speed_scale(scale);
        let due = self.pacer.frames_due(now);
        let mut stalled = false;
        let mut failure = None;
        for i in 0..due {
            let frame = driver.current_frame();
            let allowed = self.frames_limit.is_none_or(|l| l.input_allowed(frame));
            let port = if allowed { local } else { PortState::default() };
            let input = netplay_input_from_port(port, self.analog);
            let mut host = CoreRollbackHost {
                core: &mut *core,
                ram_crc: &mut self.ram_crc,
                spectator,
            };
            match driver.tick(&[input], &mut host, i + 1 == due) {
                Ok(TickOutcome::Stalled) => stalled = true,
                Ok(TickOutcome::Advanced { .. }) => stalled = false,
                Ok(TickOutcome::Waiting) => {}
                Err(err) => {
                    failure = Some(err.to_string());
                    break;
                }
            }
        }
        let events = driver.drain_events();
        let (confirmed, head) = (driver.confirmed_frame(), driver.head_frame());
        self.ram_crc.prune(confirmed);
        if let Some(message) = failure {
            self.fail(message, now);
            self.after_lobby(now);
            return due;
        }
        for event in events {
            self.on_driver_event(event);
        }
        if due > 0 {
            self.track_stall(stalled, now);
        }
        self.check_frames_limit(confirmed, head, now);
        self.update_stats(now);
        due
    }

    fn on_driver_event(&mut self, event: DriverEvent) {
        match event {
            DriverEvent::Disconnected { peer } => {
                let nick = self.nicks.get(&peer).cloned().unwrap_or_default();
                self.stats.peer_left(&nick);
                self.outbox.push(NetplayEvent::PeerLeft { nick });
                self.touch();
            }
            DriverEvent::DesyncDetected { frame, .. } if self.stats.desync_frame.is_none() => {
                tracing::warn!(frame, "netplay desync detected");
                self.outbox.push(NetplayEvent::Desync { frame });
                self.stats.desync_frame = Some(frame);
                self.touch();
            }
            _ => {}
        }
    }

    fn check_frames_limit(&mut self, confirmed: i32, head: i32, now: Instant) {
        let Some(limit) = self.frames_limit.as_mut() else {
            return;
        };
        if let Some(line) = limit.check(confirmed, head, &self.ram_crc) {
            self.print_line = Some(line);
            self.exit_at = Some((now, None));
        }
    }
}
