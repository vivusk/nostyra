//! Simulated PCs for the launcher cloud tests: each [`TestPc`] has its own data root,
//! session slot and cloud state (the launcher half of `AppState`), all syncing with
//! one storage (the in-memory fake or a Local folder). Unit folders differ per PC
//! (other ROM file name): the unit key is the content sha1.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cloud_sync::providers::fake_cloud_provider::FakeCloudProvider;
use cloud_sync::{SyncOptions, SyncScope, SyncUnit};
use tempfile::TempDir;

use crate::app_state::ActiveSession;
use crate::cloud_game_result::SyncGameResult;
use crate::cloud_game_sync::{CloudCtx, CloudHost};
use crate::cloud_provider_factory::CloudAccount;
use crate::cloud_sync_jobs::{run_game_job, run_within_budget, GameSyncRequest};
use crate::cloud_sync_state::{CloudSyncState, JobKind};
use crate::cloud_unit_guard::{game_sync_units, session_unit_keys};
use crate::commands::game_data_target::GameDataTarget;

/// Content sha1 of test game A / B.
pub const SHA1_A: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";
pub const SHA1_B: &str = "0123456789abcdef0123456789abcdef01234567";
/// A system without shared memory cards.
pub const SYSTEM: &str = "testsys";

/// The PS2 system id (shared memory-card unit), from the catalog.
pub fn ps2_system() -> &'static str {
    system_catalog::PCEE2.systems[0]
}

pub struct TestPc {
    pub temp: TempDir,
    pub label: &'static str,
    pub session: Mutex<Option<ActiveSession>>,
    pub cloud: CloudSyncState,
}

/// One simulated PC, shareable with worker threads.
#[derive(Clone)]
pub struct TestHost(pub Arc<TestPc>);

impl CloudHost for TestHost {
    fn ctx(&self) -> CloudCtx<'_> {
        CloudCtx {
            data_root: self.0.temp.path(),
            session: &self.0.session,
            cloud: &self.0.cloud,
        }
    }

    fn notify(&self, _event: &str, _payload: serde_json::Value) {}
}

impl TestHost {
    pub fn new(label: &'static str) -> Self {
        Self(Arc::new(TestPc {
            temp: tempfile::tempdir().unwrap(),
            label,
            session: Mutex::new(None),
            cloud: CloudSyncState::default(),
        }))
    }

    pub fn root(&self) -> &Path {
        self.0.temp.path()
    }

    pub fn cloud(&self) -> &CloudSyncState {
        &self.0.cloud
    }

    /// A game on this PC (ROM file named after the PC: other folder name).
    pub fn game(&self, system: &str, sha1: &str) -> GameDataTarget {
        GameDataTarget {
            system: system.to_string(),
            rom_path: PathBuf::from(format!("roms/{} copy.rom", self.0.label)),
            content_sha1: sha1.to_string(),
            missing: false,
        }
    }

    pub fn units(&self, target: &GameDataTarget) -> Vec<SyncUnit> {
        game_sync_units(self.root(), target).unwrap()
    }

    pub fn save_dir(&self, target: &GameDataTarget) -> PathBuf {
        target.saves_dir(self.root()).unwrap()
    }

    pub fn write(&self, target: &GameDataTarget, name: &str, bytes: &[u8]) {
        let dir = self.save_dir(target);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    pub fn read(&self, target: &GameDataTarget, name: &str) -> Option<Vec<u8>> {
        std::fs::read(self.save_dir(target).join(name)).ok()
    }

    pub fn request(
        &self,
        account: &CloudAccount,
        target: &GameDataTarget,
        options: SyncOptions,
    ) -> GameSyncRequest {
        GameSyncRequest {
            game_id: 1,
            account: account.clone(),
            scope: SyncScope::default(),
            units: self.units(target),
            options,
        }
    }

    /// Full sync now (the post-session push / Detail resolution path).
    pub fn sync_both(&self, account: &CloudAccount, target: &GameDataTarget) -> SyncGameResult {
        let request = self.request(account, target, SyncOptions::both());
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        run_game_job(self, JobKind::Push, deadline, stop, &request)
    }

    /// The pre-play path with `options` (pull only) and the 15 s budget.
    pub fn pre_play(
        &self,
        account: &CloudAccount,
        target: &GameDataTarget,
        options: SyncOptions,
    ) -> SyncGameResult {
        let request = self.request(account, target, options);
        run_within_budget(self, request, Duration::from_secs(15))
    }

    /// Marks `target` as running (what `play_game` stores).
    pub fn start_session(&self, target: &GameDataTarget) {
        *self.0.session.lock().unwrap() = Some(ActiveSession {
            game_id: 99,
            unit_keys: session_unit_keys(target),
        });
    }
}

/// An account over the in-memory fake.
pub fn fake_account(fake: &Arc<FakeCloudProvider>) -> CloudAccount {
    CloudAccount {
        provider: fake.clone(),
        account_key: "fake-0123456789abcdef".to_string(),
        label: "fake".to_string(),
    }
}

/// Generation of the committed manifest of `unit` in the fake (0 = none).
pub fn generation(fake: &FakeCloudProvider, unit: &SyncUnit) -> u64 {
    let path = format!("{}manifest.json", unit.remote_prefix());
    fake.get(&path)
        .and_then(|bytes| cloud_sync::RemoteManifest::parse(&bytes).ok())
        .map_or(0, |manifest| manifest.generation)
}
