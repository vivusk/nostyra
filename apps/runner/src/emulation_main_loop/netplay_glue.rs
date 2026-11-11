//! Window loop ↔ netplay session (M20 phase 04). Attaching sets the gate (guests
//! and spectators write nothing, a guest's `--load-state` is dropped, achievements
//! go softcore). Per redraw `netplay_frames` replaces rewind / fast-forward /
//! run-ahead / `LoopTiming` pacing: local port (neutral while the menu is open),
//! lobby poll, host Start, Resync, then the frames the session's own pacer says are
//! due. `netplay_after_frames` forwards launcher events, prints the frames-limit
//! line and closes the window when the session ended.

mod netplay_events;
mod netplay_hooks;

use std::time::Instant;

use input_mapping::{JoypadButton, NetplayInputSource, PortState};
use libretro_host::InputSource;
use netplay_session::lobby_state::LobbyPhase;

use self::netplay_hooks::NetplayLoopHooks;
use super::EmulationApp;
use crate::netplay_gate::menu_mode::local_port;
use crate::netplay_gate::{GatedAction, NetplayGate, NetplaySeat};
use crate::netplay_launch::NetplayAttach;
use crate::netplay_overlay::NetplayOverlay;
use crate::netplay_runtime::session_core::SessionCore;
use crate::netplay_runtime::NetplayRuntime;

/// Redraw pause when no netplay frame was due.
const IDLE_REDRAW_SLEEP: std::time::Duration = std::time::Duration::from_millis(1);

/// The netplay half of the window loop.
pub(super) struct NetplayLoop {
    pub runtime: NetplayRuntime,
    pub gate: NetplayGate,
    input: NetplayInputSource,
    analog: bool,
    scripted: Option<PortState>,
    pub hud_on: bool,
    pub window_hidden: bool,
    pub overlay: NetplayOverlay,
    /// Start was down on the last redraw (host presses Start on the edge).
    start_held: bool,
    /// Menu "Resync" waits for the next redraw (it needs the core).
    pub resync_requested: bool,
}

impl EmulationApp<'_> {
    /// Hands the session prepared before the window to the loop.
    pub fn attach_netplay(&mut self, attach: NetplayAttach) {
        let gate = NetplayGate::new(attach.seat);
        let host = attach.seat == NetplaySeat::Host;
        self.saves.set_persist(host); // guests play on the host's save (F14)
        if !host {
            self.startup_load = None; // a guest's state would be replaced anyway
        }
        if let Some(achievements) = self.achievements.as_mut() {
            achievements.drop_to_softcore("netplay");
        }
        self.netplay = Some(NetplayLoop {
            runtime: attach.runtime,
            gate,
            input: attach.input,
            analog: attach.analog,
            scripted: attach.scripted,
            hud_on: attach.hud_on,
            window_hidden: attach.window_hidden,
            overlay: NetplayOverlay::default(),
            start_held: false,
            resync_requested: false,
        });
    }

    /// The session's netplay seat (inactive outside netplay).
    pub(super) fn netplay_gate(&self) -> NetplayGate {
        self.netplay
            .as_ref()
            .map(|np| np.gate.clone())
            .unwrap_or_default()
    }

    /// Whether a netplay session drives this window.
    pub(super) fn netplay_active(&self) -> bool {
        self.netplay.is_some()
    }

    /// This redraw's netplay work (replaces the local-play emulation paths).
    pub(super) fn netplay_frames(&mut self) -> anyhow::Result<()> {
        let now = Instant::now();
        let gate = self.netplay_gate();
        let held = [
            (self.input.rewind_held(), GatedAction::Rewind),
            (self.input.fast_forward_held(), GatedAction::FastForward),
        ];
        if let Some(refused) = held.iter().find(|h| h.0).and_then(|h| gate.refusal(h.1)) {
            self.status.show_while_held(refused, now);
        }
        self.input.poll(); // keyboard + pads + turbo into the local snapshot
        let menu_open = self.menu.is_open();
        let Some(np) = self.netplay.as_mut() else {
            return Ok(());
        };
        let input = &self.input;
        let local = local_port(menu_open, np.scripted, || input.local_port_state());
        let start = local.buttons & JoypadButton::Start.mask() != 0;
        let start_pressed = start && !np.start_held;
        np.start_held = start;
        let resync = std::mem::take(&mut np.resync_requested);
        let mut hooks = NetplayLoopHooks {
            timing: &mut self.timing,
            saves: &mut self.saves,
            achievements: &mut self.achievements,
            turbo_clock: &self.turbo_clock,
        };
        let mut core = SessionCore {
            session: &mut self.session,
            video: &mut self.frame,
            input: &mut np.input,
            analog: np.analog,
            hooks: &mut hooks,
        };
        np.runtime.poll_lobby(&mut core, now);
        let lobby_open = matches!(
            np.runtime.phase(),
            LobbyPhase::Connecting | LobbyPhase::Handshaking | LobbyPhase::InLobby
        );
        if !lobby_open {
            self.startup_load = None; // the host's state is loaded before Start or never
        }
        if start_pressed && *np.runtime.phase() == LobbyPhase::InLobby {
            np.runtime.press_start(&mut core, now);
        }
        if resync {
            np.runtime.request_resync(&mut core, now);
        }
        let started = Instant::now();
        let due = np.runtime.run_due_frames(&mut core, local, now);
        let done = Instant::now();
        self.emulate_stats.record(due, done - started, done, 0);
        if due == 0 {
            // Immediate present mode (audio-sync) would spin: the netplay pacer
            // never blocks on the audio queue.
            std::thread::sleep(IDLE_REDRAW_SLEEP);
        }
        Ok(())
    }
}
