//! Fixtures of the offline-cache session tests: NES runs one after another over the
//! same data dir, each behind `SessionBackend::with_offline_cache` (inline dispatch),
//! so a first run's answers are what a later offline run is served.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use achievements::{FakeTransport, RequestDispatch, ServerTransport};
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use super::achievement_http_worker::SessionBackend;
use super::achievement_session_test_support::{fake_with_set, login, Harness};
use super::{AchievementSession, SessionStart};

/// A fake whose every request kind answers "no response" (server unreachable).
pub(super) fn offline_fake() -> Arc<FakeTransport> {
    let fake = fake_with_set();
    for kind in "login2 achievementsets startsession awardachievement ping".split(' ') {
        fake.respond(kind, -2, "offline");
    }
    fake
}

/// A second NES game (other bytes, so another RA hash) in `dir`.
pub(super) fn other_rom(dir: &Path) -> PathBuf {
    let rom = dir.join("other.rom");
    std::fs::write(&rom, vec![0x24u8; 0x4000]).unwrap();
    rom
}

/// Starts a session of `rom` over `fake` behind the offline cache of `data`; with
/// `hardcore` the launch is hardcore (hidden UI flag passed on).
pub(super) fn cached_run(
    fake: Arc<FakeTransport>,
    data: tempfile::TempDir,
    rom: &Path,
    hardcore: bool,
) -> Harness {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let config = AchievementsConfig {
        hardcore,
        ..AchievementsConfig::default()
    };
    let start = SessionStart {
        config: &config,
        env_credentials: login(),
        data_root: data.path(),
        content_id: "ABCDEF",
        system: &system_catalog::NES,
        rom,
        patched: None,
        core_stem: "test_libretro",
        core_version: "1.0",
        library_name: "Mesen",
        core_options: &[],
        hardcore_ui: hardcore,
    };
    let transport: Arc<dyn ServerTransport> = fake.clone();
    let root = data.path().to_path_buf();
    let backend = move |_: &str| {
        SessionBackend::with_offline_cache(transport, RequestDispatch::Inline, &root)
    };
    let emit = Box::new(move |e: &RunnerEvent| sink.borrow_mut().push(e.clone()));
    let session = AchievementSession::start(start, backend, emit).expect("session");
    let ram = Rc::new((0..0x800).map(|_| Cell::new(0u8)).collect::<Vec<_>>());
    Harness {
        session,
        fake,
        data,
        ram,
        events,
    }
}

/// Ends a run; its data dir (cache, journal) is handed to the next run.
pub(super) fn finish(run: Harness) -> tempfile::TempDir {
    let Harness { session, data, .. } = run;
    session.shutdown();
    data
}

/// The run's `AchievementsLoaded` event, if the set loaded.
pub(super) fn loaded_event(run: &Harness) -> Option<RunnerEvent> {
    let events = run.events.borrow();
    events
        .iter()
        .find(|e| matches!(e, RunnerEvent::AchievementsLoaded { .. }))
        .cloned()
}
