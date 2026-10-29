//! An `AchievementSession` over a canned fake set of `test-assets/achievements/<system>/`
//! (written by `generate-fake-sets.py`; the fake answers the set only for the ROM's
//! hash), started exactly as the runner starts it (`AchievementSession::start`) but on
//! inline dispatch, so the set loads inside the first frame that has memory (M15 H2).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use achievements::{Credentials, FakeTransport, RequestDispatch};
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use crate::achievement_session::{AchievementSession, SessionBackend, SessionStart};
use crate::runner_real_core_suite::repo;

/// `case.json` of a canned set.
#[derive(serde::Deserialize, Debug, Clone)]
pub(crate) struct RaCase {
    pub(crate) system: String,
    pub(crate) rom: String,
    pub(crate) console_id: u32,
    pub(crate) game_id: u32,
    pub(crate) achievement_id: u32,
    pub(crate) leaderboard_id: u32,
    pub(crate) counter_address: u32,
    pub(crate) unlock_value: u8,
    pub(crate) lb_submit_value: u8,
}

impl RaCase {
    pub(crate) fn load(system: &str) -> Self {
        let text = std::fs::read_to_string(fake_set_dir(system).join("case.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    pub(crate) fn rom(&self) -> PathBuf {
        repo(&format!("test-assets/roms/{}", self.rom))
    }

    pub(crate) fn content_id(&self) -> String {
        format!("{}-ra-suite", self.system)
    }
}

/// The canned fake-server folder of `system`.
pub(crate) fn fake_set_dir(system: &str) -> PathBuf {
    repo(&format!("test-assets/achievements/{system}"))
}

/// The core facts a session start needs (`library_name`, version, host options).
pub(crate) struct CoreFacts<'a> {
    pub(crate) library_name: &'a str,
    pub(crate) version: &'a str,
    pub(crate) options: &'a [(String, String)],
}

/// A started session (an `Option`, as the runner holds it for `after_real_frame`) with
/// its fake server and the launcher events it emitted.
pub(crate) struct FakeSetSession {
    pub(crate) session: Option<AchievementSession>,
    pub(crate) fake: Arc<FakeTransport>,
    pub(crate) events: Rc<RefCell<Vec<RunnerEvent>>>,
}

/// Starts the session of `case` for `rom` (hardcore with the hidden flag forced on);
/// snapshot and journal go to `data`.
pub(crate) fn start_fake_set_session(
    case: &RaCase,
    rom: &Path,
    data: &Path,
    hardcore: bool,
    core: &CoreFacts<'_>,
) -> FakeSetSession {
    let fake = Arc::new(FakeTransport::from_dir(&fake_set_dir(&case.system)).unwrap());
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let config = AchievementsConfig {
        enabled: true,
        hardcore,
        sound: false,
        leaderboard_popups: true,
        patched_rom_fallback: true,
    };
    let content_id = case.content_id();
    let start = SessionStart {
        config: &config,
        env_credentials: Some(Credentials {
            user: "Tester".into(),
            token: "tok123".into(),
        }),
        data_root: data,
        content_id: &content_id,
        system: system_catalog::system(&case.system).unwrap(),
        rom,
        patched: None,
        core_stem: "test_core",
        core_version: core.version,
        library_name: core.library_name,
        core_options: core.options,
        hardcore_ui: hardcore,
    };
    let transport = fake.clone();
    let backend = move |_: &str| SessionBackend::new(transport, RequestDispatch::Inline);
    let emit = Box::new(move |e: &RunnerEvent| sink.borrow_mut().push(e.clone()));
    let session = AchievementSession::start(start, backend, emit).expect("session");
    FakeSetSession {
        session: Some(session),
        fake,
        events,
    }
}
