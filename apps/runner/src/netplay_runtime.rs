//! One netplay session inside the runner (M20 phase 04): the room socket, the
//! lobby state machine, the ggrs rollback driver and its own wall-clock pacer, the
//! frames-limit test hook and what the launcher / overlay need to know. Core
//! independent: the emulator is reached through [`NetplayCore`] (real core in the
//! window loop, a fake in tests). Never logs room codes, tokens, ids or addresses.
//!
//! Per redraw the loop calls [`NetplayRuntime::poll_lobby`] (peers, lobby
//! messages, sync, start / resync) and, once running, `run_due_frames`.

#[cfg(test)]
pub(crate) mod fake_netplay_core;
pub(crate) mod fingerprint_source;
mod frame_driver;
mod frames_limit;
mod hud_stats;
#[cfg(test)]
mod in_process_session_tests;
mod lobby_pump;
#[cfg(test)]
pub(crate) mod local_mesh;
pub(crate) mod netplay_core;
pub(crate) mod netplay_exit;
pub(crate) mod peer_socket;
mod ram_crc_history;
mod rollback_host;
mod runtime_view;
pub(crate) mod save_sync;
pub(crate) mod session_core;

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use ipc_protocol::NetplayEvent;
use netplay_session::lobby_link::LobbyLink;
use netplay_session::lobby_state::{EndReason, LobbyConfig, LobbyEvent, LobbyState};
use netplay_session::peer_handles::PeerHandles;
use netplay_session::rollback_driver::RollbackDriver;
use netplay_session::room_api_types::SeatRole;
use netplay_session::session_fingerprint::SessionFingerprint;
use netplay_session::PeerKey;

use self::frames_limit::FramesLimit;
use self::netplay_exit::NetplayExit;
use self::peer_socket::PeerSocket;
use self::ram_crc_history::FrameCrcs;
use crate::frame_pacer::FramePacer;

/// Frames per redraw at most: ≥ ggrs `max_frames_behind` (10), spectator catch-up.
pub(crate) const NETPLAY_MAX_FRAMES_PER_TICK: u32 = 12;

/// Static facts of this peer's session.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeOptions {
    pub role: SeatRole,
    pub nickname: String,
    pub fingerprint: SessionFingerprint,
    pub sram_len: u32,
    pub rtc_len: Option<u32>,
    /// `--netplay-input-delay N`; `None` = auto from the lobby ping.
    pub input_delay: Option<usize>,
    /// Host test flag: Start once the room is full and this many spectators joined.
    pub autostart: Option<u8>,
    pub frames_limit: Option<i32>,
    /// The core is offered as Experimental (badge + warning).
    pub experimental: bool,
    /// Host only: shown large in the waiting room (never logged or sent over IPC).
    pub room_code: Option<String>,
}

pub(crate) struct NetplayRuntime {
    socket: Box<dyn PeerSocket>,
    link: LobbyLink,
    lobby: LobbyState,
    driver: Option<RollbackDriver>,
    handles: Option<PeerHandles>,
    options: RuntimeOptions,
    analog: bool,
    pacer: FramePacer,
    ram_crc: FrameCrcs,
    frames_limit: Option<FramesLimit>,
    warmup_reported: bool,
    /// Nicknames seen in the roster (a `PeerLeft` after the roster dropped them).
    nicks: HashMap<PeerKey, String>,
    /// Lobby events of the current redraw (scratch queue, capacity reused).
    events: VecDeque<LobbyEvent>,
    /// A local failure (socket closed, core refused the sync): overrides the lobby.
    local_end: Option<EndReason>,
    /// Why the session ended (end screen).
    end_reason: Option<EndReason>,
    /// `--netplay-frames-limit` result line, printed once by the loop.
    print_line: Option<String>,
    /// When the ended session closes the runner, and with which exit.
    exit_at: Option<(Instant, Option<NetplayExit>)>,
    outbox: Vec<NetplayEvent>,
    stats: hud_stats::HudStats,
    /// Bumped on every change the overlay draws (repaint on change only).
    revision: u64,
}

impl NetplayRuntime {
    pub(crate) fn new(socket: Box<dyn PeerSocket>, options: RuntimeOptions, now: Instant) -> Self {
        let link = LobbyLink::new(socket.lobby_channel());
        let lobby = LobbyState::new(LobbyConfig {
            role: options.role,
            nickname: options.nickname.clone(),
            fingerprint: options.fingerprint.clone(),
            sram_len: options.sram_len,
            rtc_len: options.rtc_len,
            input_delay: options.input_delay,
        });
        let fps = f64::from(options.fingerprint.fps_milli) / 1000.0;
        let mut ram_crc = FrameCrcs::default();
        let settle = match options.role {
            SeatRole::Spectator => 0,
            _ => frames_limit::SETTLE_FRAMES,
        };
        let frames_limit = options.frames_limit.map(|n| FramesLimit::new(n, settle));
        if let Some(limit) = &frames_limit {
            ram_crc.keep(limit.target());
        }
        Self {
            socket,
            link,
            lobby,
            driver: None,
            handles: None,
            analog: options.fingerprint.analog,
            options,
            pacer: FramePacer::with_cap(fps, NETPLAY_MAX_FRAMES_PER_TICK),
            ram_crc,
            frames_limit,
            warmup_reported: false,
            nicks: HashMap::new(),
            events: VecDeque::new(),
            local_end: None,
            end_reason: None,
            print_line: None,
            exit_at: None,
            outbox: Vec::new(),
            stats: hud_stats::HudStats::new(now),
            revision: 1,
        }
    }

    pub(crate) fn role(&self) -> SeatRole {
        self.options.role
    }

    /// Launcher events since the last call.
    pub(crate) fn take_events(&mut self) -> Vec<NetplayEvent> {
        std::mem::take(&mut self.outbox)
    }

    /// The runner must close now: `Some(None)` = exit 0, `Some(Some(e))` = exit
    /// with `e.code` (host left, refused, error).
    pub(crate) fn exit_due(&self, now: Instant) -> Option<Option<NetplayExit>> {
        let (at, exit) = self.exit_at.as_ref()?;
        (now >= *at).then(|| exit.clone())
    }

    /// The `ram_crc=` line of `--netplay-frames-limit`, once.
    pub(crate) fn take_print_line(&mut self) -> Option<String> {
        self.print_line.take()
    }

    /// Local quit (window closed, menu Quit): tells the peers once, then queues
    /// `Ended` (`Quit`, or `Error` after [`Self::fail`]).
    pub(crate) fn quit(&mut self, now: Instant) {
        let effects = self.lobby.apply(LobbyEvent::Quit, now);
        for effect in effects {
            if let netplay_session::lobby_state::LobbyEffect::Send { to, msg } = effect {
                let _ = self.link.send(&to, &msg);
            }
        }
        self.after_lobby(now);
    }

    /// Ends the session locally with an error (peers told through `Quit`).
    fn fail(&mut self, message: String, now: Instant) {
        tracing::warn!("netplay session failed: {message}");
        if self.local_end.is_none() {
            self.local_end = Some(EndReason::Error(message));
            self.quit(now);
        }
    }

    fn is_spectator(&self) -> bool {
        self.options.role == SeatRole::Spectator
    }

    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
