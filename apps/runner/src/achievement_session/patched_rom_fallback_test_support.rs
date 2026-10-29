//! Fixtures of the patched ROM fallback tests: an NES session whose core received
//! soft-patched bytes (inline dispatch over the fake server), with both hashes.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use achievements::{content_hash, form_field, request_kind, FakeTransport, RequestDispatch};
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use super::achievement_http_worker::SessionBackend;
use super::achievement_session_test_support::{login, nes_rom, Harness};
use super::{AchievementSession, SessionStart};

/// A patched NES session and the hashes of its original file and patched bytes.
pub(super) struct PatchedRun {
    pub(super) run: Harness,
    pub(super) original_hash: String,
    pub(super) patched_hash: String,
}

/// Starts an NES session over `fake` with soft-patched bytes (`nes_rom` on disk);
/// `hardcore` launches hardcore with the hidden UI flag passed on.
pub(super) fn patched_run(
    fake: Arc<FakeTransport>,
    config: &AchievementsConfig,
    hardcore: bool,
) -> PatchedRun {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let patched = vec![0x43u8; 0x4000];
    let console = u32::from(system_catalog::NES.ra_console_id.unwrap());
    let original_hash = content_hash(console, &rom, None).unwrap();
    let patched_hash = content_hash(console, &rom, Some(&patched)).unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let config = AchievementsConfig {
        hardcore,
        ..config.clone()
    };
    let start = SessionStart {
        config: &config,
        env_credentials: login(),
        data_root: data.path(),
        content_id: "ABCDEF",
        system: &system_catalog::NES,
        rom: &rom,
        patched: Some(patched),
        core_stem: "test_libretro",
        core_version: "1.0",
        library_name: "Mesen",
        core_options: &[],
        hardcore_ui: hardcore,
    };
    let transport = fake.clone();
    let backend = move |_: &str| SessionBackend::new(transport, RequestDispatch::Inline);
    let emit = Box::new(move |e: &RunnerEvent| sink.borrow_mut().push(e.clone()));
    let session = AchievementSession::start(start, backend, emit).expect("session");
    let ram = Rc::new((0..0x800).map(|_| Cell::new(0u8)).collect::<Vec<_>>());
    let run = Harness {
        session,
        fake,
        data,
        ram,
        events,
    };
    PatchedRun {
        run,
        original_hash,
        patched_hash,
    }
}

/// The `m=` hash of every request of `kind`, in order.
pub(super) fn hashes_of(fake: &FakeTransport, kind: &str) -> Vec<String> {
    fake.posts()
        .iter()
        .filter(|post| request_kind(post) == kind)
        .filter_map(|post| form_field(post, "m").map(str::to_owned))
        .collect()
}
