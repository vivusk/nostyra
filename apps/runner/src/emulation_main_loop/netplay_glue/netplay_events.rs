//! After a redraw's netplay frames: launcher events (`Netplay{…}`), the
//! frames-limit line on stdout, the end of the session (window closed; host left /
//! refused / error become the runner's exit code), and the local quit.

use std::time::Instant;

use ipc_protocol::RunnerEvent;
use winit::event_loop::ActiveEventLoop;

use super::super::EmulationApp;
use crate::ipc_event_writer::write_stdout;

impl EmulationApp<'_> {
    pub(in crate::emulation_main_loop) fn netplay_after_frames(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) {
        let Some(np) = self.netplay.as_mut() else {
            return;
        };
        for event in np.runtime.take_events() {
            self.reporter.emit(&RunnerEvent::Netplay(event));
        }
        if let Some(line) = np.runtime.take_print_line() {
            write_stdout(&format!("{line}\n"));
        }
        if let Some(exit) = np.runtime.exit_due(Instant::now()) {
            if let Some(exit) = exit {
                self.error.get_or_insert(exit.into());
            }
            event_loop.exit();
        }
    }

    /// Window closed / menu Quit: tell the peers once (before the session drops).
    pub(in crate::emulation_main_loop) fn netplay_quit(&mut self) {
        if let Some(np) = self.netplay.as_mut() {
            np.runtime.quit(Instant::now());
            for event in np.runtime.take_events() {
                self.reporter.emit(&RunnerEvent::Netplay(event));
            }
        }
    }

    /// Menu "Resync" (host): runs on the next redraw, with the core at hand.
    pub(in crate::emulation_main_loop) fn request_netplay_resync(&mut self) {
        if let Some(np) = self.netplay.as_mut() {
            np.resync_requested = true;
        }
        self.close_menu();
    }

    /// Session line of the netplay menu: "Host, 2 players, ping 40 ms".
    pub(in crate::emulation_main_loop) fn netplay_menu_info(&self) -> Option<(String, bool)> {
        let np = self.netplay.as_ref()?;
        let runtime = &np.runtime;
        let role = format!("{:?}", runtime.role());
        let players = runtime
            .roster_lines()
            .iter()
            .filter(|m| m.role.is_player())
            .count();
        let ping = runtime
            .hud()
            .ping_ms
            .map(|ms| format!(", ping {ms} ms"))
            .unwrap_or_default();
        Some((
            format!("{role}, {players} players{ping}"),
            runtime.is_host(),
        ))
    }
}
