//! RetroAchievements in the runner (M15): one [`AchievementSession`] per game, owned
//! by the session thread (window loop or headless run). It owns the `rc_client`
//! (softcore unless hardcore is effective), its HTTP worker, the lazily built memory
//! regions, the offline-journal bookkeeping and the launcher outputs (IPC events,
//! the per-game snapshot) and the achievement overlay (popups, indicators, badges,
//! chime, composed with the status line).
//!
//! No session at all (zero cost) when achievements are off, nobody is logged in or
//! the system has no RA console. Shutdown order (M15 D4): pending results routed →
//! client shutdown (fire −2 → destroy → join/detach the worker) → memory regions
//! dropped, all before the core unloads (the owner drops this before `CoreSession`).

use achievements::{AchievementClient, HashRequest, LazyMemoryRegions};
use ipc_protocol::RunnerEvent;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use self::achievement_event_routing::SessionRouter;
use self::offline_session_watch::OfflineSessionWatch;
use crate::achievement_overlay::{AchievementChime, OverlayComposer};
use crate::hardcore_gate::HardcoreGate;

pub mod achievement_credentials;
mod achievement_event_routing;
mod achievement_frame_hooks;
mod achievement_http_worker;
mod achievement_journal_bridge;
mod achievement_overlay_frame;
#[cfg(test)]
mod achievement_overlay_frame_tests;
mod achievement_overlay_routing;
#[cfg(test)]
mod achievement_session_progress_tests;
mod achievement_session_start;
#[cfg(test)]
mod achievement_session_test_support;
#[cfg(test)]
mod achievement_session_tests;
mod achievement_snapshot_writer;
mod achievement_state_progress;
mod hardcore_option_check;
mod hardcore_session_gate;
#[cfg(test)]
mod hardcore_session_tests;
#[cfg(test)]
mod offline_cache_session_test_support;
#[cfg(test)]
mod offline_cache_session_tests;
mod offline_session_watch;
mod patched_rom_fallback;
#[cfg(test)]
mod patched_rom_fallback_test_support;
#[cfg(test)]
mod patched_rom_fallback_tests;

pub use self::achievement_frame_hooks::after_real_frame;
#[cfg(test)]
pub use self::achievement_http_worker::SessionBackend;
pub use self::achievement_session_start::{start_for_runner, SessionStart};
pub use self::achievement_state_progress::progress_of;

/// Launcher event output (`SessionReporter::emit` in the runner, a log in tests).
pub type EventSink = Box<dyn Fn(&RunnerEvent)>;

/// One game's achievement session.
pub struct AchievementSession {
    /// `None` once stopped (token rejected) or for an unhashable format: inert.
    client: Option<AchievementClient>,
    /// Shared with the client's memory reader; dropped after the client.
    memory: Rc<RefCell<LazyMemoryRegions>>,
    console_id: u32,
    rom: PathBuf,
    router: SessionRouter,
    /// The core never exposed memory within the frame budget (toast shown once).
    memory_unavailable: bool,
    /// Paints the overlay (status line + achievement layers) for the window.
    composer: OverlayComposer,
    /// Unlock chime (window sessions with `sound` on).
    chime: Option<AchievementChime>,
    /// Game load / disc change waiting for memory regions: rc_client checks every
    /// address with a memory read when a set loads, and regions exist only after the
    /// first real frame (D6), so the request starts once they are ready.
    deferred: Option<DeferredLoad>,
    /// Hardcore rules on (shared with the state slots and rewind); lifted when the
    /// session drops to softcore.
    hardcore: HardcoreGate,
    /// Answers served by the offline cache (softcore, "unlocks will sync" popup).
    offline: OfflineSessionWatch,
}

/// A set request held until the memory regions are ready.
enum DeferredLoad {
    Identify(HashRequest),
    /// Disc swap: hash of the inserted disc.
    ChangeMedia(String),
}

impl AchievementSession {
    /// Every popup text since the last call (headless: logged).
    pub fn take_notices(&mut self) -> Vec<String> {
        self.router.overlay.take_log()
    }

    /// Delivers arrived HTTP results, then routes observations, completions and
    /// events. Stops the client when the server rejected the token.
    fn pump(&mut self) {
        self.router.overlay.badges.poll();
        if let Some(client) = &self.client {
            client.complete_ready_requests();
        }
        // Before routing too: a set served from the cache loads as softcore.
        self.follow_offline_cache();
        if let Some(client) = &self.client {
            self.router.route(client);
        }
        self.follow_offline_cache();
        self.follow_client_hardcore();
        if self.router.login_expired && self.client.is_some() {
            self.stop();
        }
    }

    /// Ends the client in the D4 order and drops the memory regions; the session stays
    /// (inert) so callers need no special case.
    fn stop(&mut self) {
        if let Some(client) = self.client.take() {
            client.complete_ready_requests();
            self.router.route(&client);
            let report = client.shutdown();
            tracing::info!(?report, "achievement client stopped");
        }
        if self.hardcore.lift() {
            tracing::info!("hardcore gates lifted: achievement client stopped");
        }
        self.memory.borrow_mut().reset();
    }

    /// Before the core unloads (window `finish`, end of a headless run).
    pub fn shutdown(mut self) {
        self.stop();
    }
}
