//! Fixtures of the `AchievementSession` tests: a session over the fake server
//! without a core (an NES set read from 2 KiB of system RAM, the `rc_libretro`
//! fallback), inline dispatch or the real HTTP worker, with its launcher events.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use achievements::{
    fake_game_set_json, Credentials, FakeTransport, PendingJournal, RequestDispatch,
};
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use super::achievement_http_worker::SessionBackend;
use super::{AchievementSession, SessionStart};

pub(super) const BYTE_ACH: u32 = 101; // 0xH0010=5
pub(super) const HITS_ACH: u32 = 102; // 0xH0030=1.10.

/// One session under test: its data dir, NES system RAM and launcher events.
pub(super) struct Harness {
    pub(super) session: AchievementSession,
    pub(super) fake: Arc<FakeTransport>,
    pub(super) data: tempfile::TempDir,
    pub(super) ram: Rc<Vec<Cell<u8>>>,
    pub(super) events: Rc<RefCell<Vec<RunnerEvent>>>,
}

pub(super) fn fake_with_set() -> Arc<FakeTransport> {
    let fake = Arc::new(FakeTransport::new());
    let set = fake_game_set_json(
        1234,
        "Test Game",
        &[(BYTE_ACH, "0xH0010=5"), (HITS_ACH, "0xH0030=1.10.")],
        &[],
    );
    fake.respond("achievementsets", 200, &set);
    fake
}

pub(super) fn login() -> Option<Credentials> {
    Some(Credentials {
        user: "Tester".into(),
        token: "tok123".into(),
    })
}

/// Hardcore facts of a launch: options the host answers and the hidden UI flag.
#[derive(Default)]
pub(super) struct HardcoreLaunch<'a> {
    pub(super) core_options: &'a [(String, String)],
    pub(super) hardcore_ui: bool,
}

pub(super) fn try_start(
    fake: &Arc<FakeTransport>,
    dispatch: RequestDispatch,
    system_rom: (&'static system_catalog::SystemSpec, &Path),
    config: &AchievementsConfig,
    data: &Path,
    credentials: Option<Credentials>,
) -> (Option<AchievementSession>, Rc<RefCell<Vec<RunnerEvent>>>) {
    let launch = HardcoreLaunch::default();
    let (system, rom) = system_rom;
    try_start_with(
        fake,
        dispatch,
        (system, rom, config),
        data,
        credentials,
        launch,
    )
}

pub(super) fn try_start_with(
    fake: &Arc<FakeTransport>,
    dispatch: RequestDispatch,
    (system, rom, config): (
        &'static system_catalog::SystemSpec,
        &Path,
        &AchievementsConfig,
    ),
    data: &Path,
    credentials: Option<Credentials>,
    launch: HardcoreLaunch<'_>,
) -> (Option<AchievementSession>, Rc<RefCell<Vec<RunnerEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let start = SessionStart {
        config,
        env_credentials: credentials,
        data_root: data,
        content_id: "ABCDEF",
        system,
        rom,
        patched: None,
        core_stem: "test_libretro",
        core_version: "1.0",
        library_name: "Mesen",
        core_options: launch.core_options,
        hardcore_ui: launch.hardcore_ui,
    };
    let transport = fake.clone();
    let backend = move |_: &str| SessionBackend::new(transport, dispatch);
    let emit = Box::new(move |e: &RunnerEvent| sink.borrow_mut().push(e.clone()));
    (AchievementSession::start(start, backend, emit), events)
}

pub(super) fn nes_rom(dir: &Path) -> PathBuf {
    let rom = dir.join("game.rom"); // NES hashing does not depend on the extension
    std::fs::write(&rom, vec![0x42u8; 0x4000]).unwrap();
    rom
}

pub(super) fn harness(fake: Arc<FakeTransport>, dispatch: RequestDispatch) -> Harness {
    harness_with(
        fake,
        dispatch,
        &AchievementsConfig::default(),
        HardcoreLaunch::default(),
    )
}

/// [`harness`] with a given config and hardcore launch facts.
pub(super) fn harness_with(
    fake: Arc<FakeTransport>,
    dispatch: RequestDispatch,
    config: &AchievementsConfig,
    launch: HardcoreLaunch<'_>,
) -> Harness {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let nes = (&system_catalog::NES, rom.as_path(), config);
    let (session, events) = try_start_with(&fake, dispatch, nes, data.path(), login(), launch);
    let ram = Rc::new((0..0x800).map(|_| Cell::new(0u8)).collect::<Vec<_>>());
    Harness {
        session: session.expect("session"),
        fake,
        data,
        ram,
        events,
    }
}

impl Harness {
    pub(super) fn frame(&mut self) {
        let ram = self.ram.clone();
        // System RAM (id 2) only: NES without a memory map uses the fallback.
        let lookup = move |id: u32| (id == 2).then_some((ram.as_ptr() as *mut u8, ram.len()));
        self.session.on_real_frame(None, &lookup);
    }

    pub(super) fn frames(&mut self, n: usize) {
        (0..n).for_each(|_| self.frame());
    }

    pub(super) fn unlocked(&self) -> Vec<u32> {
        let events = self.events.borrow();
        events
            .iter()
            .filter_map(|e| match e {
                RunnerEvent::AchievementUnlocked { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Runs frames (worker mode: also waits) until the set is loaded (5 s cap); the
    /// load starts after the first frame, once memory regions exist.
    pub(super) fn wait_loaded(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.loaded() {
            assert!(Instant::now() < deadline, "set never loaded");
            self.frame();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    pub(super) fn loaded(&self) -> bool {
        let events = self.events.borrow();
        events
            .iter()
            .any(|e| matches!(e, RunnerEvent::AchievementsLoaded { .. }))
    }

    pub(super) fn journal(&self) -> PendingJournal {
        PendingJournal::new(self.data.path())
    }
}
