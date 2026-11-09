//! Two-PC simulation for executor tests: each [`Pc`] has its own data root, device
//! identity, base store and progress handle; all share one provider (a Local folder
//! or the fake). Unit folders may differ in name per PC (renamed ROM): the unit key
//! is the content sha1, not the folder.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::base_manifest_store::{account_key, BaseManifest, BaseManifestStore};
use crate::cloud_storage_provider::CloudStorageProvider;
use crate::conflict_choice::Choice;
use crate::device_identity::DeviceIdentity;
use crate::local_save_backups::unit_backups_dir;
use crate::sync_outcome::{SyncContext, SyncOptions, SyncOutcome, SyncStatus, SyncUnitError};
use crate::sync_plan_executor::sync_unit;
use crate::sync_progress::SyncProgress;
use crate::sync_scope::SyncScope;
use crate::sync_staging::join_rel;
use crate::sync_unit::SyncUnit;

/// Content sha1 of the test game.
pub(crate) const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";
/// System of the test game.
pub(crate) const SYSTEM: &str = "testsys";

/// One simulated PC.
pub(crate) struct Pc {
    pub(crate) data: TempDir,
    pub(crate) device: DeviceIdentity,
    pub(crate) bases: BaseManifestStore,
    pub(crate) progress: SyncProgress,
    pub(crate) scope: SyncScope,
}

impl Pc {
    pub(crate) fn new(name: &str) -> Self {
        let data = tempfile::tempdir().unwrap();
        let device = DeviceIdentity::load_or_create_named(data.path(), name).unwrap();
        let bases = BaseManifestStore::new(data.path(), &account_key("local_folder", "shared"));
        Self {
            data,
            device,
            bases,
            progress: SyncProgress::new(),
            scope: SyncScope::default(),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        self.data.path()
    }

    /// The test game's unit with its save folder named `folder` on this PC.
    pub(crate) fn game(&self, folder: &str) -> SyncUnit {
        let dir = self.root().join("saves").join(SYSTEM).join(folder);
        SyncUnit::game(SYSTEM, SHA1, dir).unwrap()
    }

    pub(crate) fn memcards(&self) -> SyncUnit {
        SyncUnit::ps2_memcards(self.root())
    }

    pub(crate) fn sync(
        &self,
        provider: &dyn CloudStorageProvider,
        unit: &SyncUnit,
        options: SyncOptions,
    ) -> Result<SyncOutcome, SyncUnitError> {
        let ctx = SyncContext {
            provider,
            data_root: self.root(),
            bases: &self.bases,
            device: &self.device,
            scope: self.scope,
            progress: &self.progress,
        };
        sync_unit(&ctx, unit, options)
    }

    /// Full sync that must end `Synced`.
    pub(crate) fn sync_ok(
        &self,
        provider: &dyn CloudStorageProvider,
        unit: &SyncUnit,
    ) -> SyncOutcome {
        let outcome = self.sync(provider, unit, SyncOptions::both()).unwrap();
        assert_eq!(outcome.status, SyncStatus::Synced, "{outcome:?}");
        outcome
    }

    /// Full sync resolving conflicts with `choice`.
    pub(crate) fn sync_choosing(
        &self,
        provider: &dyn CloudStorageProvider,
        unit: &SyncUnit,
        choice: Choice,
    ) -> SyncOutcome {
        let options = SyncOptions::both().with_choice(choice);
        let outcome = self.sync(provider, unit, options).unwrap();
        assert_eq!(outcome.status, SyncStatus::Synced, "{outcome:?}");
        outcome
    }

    pub(crate) fn path(&self, unit: &SyncUnit, rel: &str) -> PathBuf {
        join_rel(unit.local_dir(), rel)
    }

    pub(crate) fn write(&self, unit: &SyncUnit, rel: &str, bytes: &[u8]) {
        let path = self.path(unit, rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    pub(crate) fn read(&self, unit: &SyncUnit, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(unit, rel)).ok()
    }

    pub(crate) fn remove(&self, unit: &SyncUnit, rel: &str) {
        std::fs::remove_file(self.path(unit, rel)).unwrap();
    }

    pub(crate) fn base(&self, unit: &SyncUnit) -> Option<BaseManifest> {
        self.bases.load(unit).unwrap()
    }

    /// Backup folder names of `unit`, oldest first.
    pub(crate) fn backup_dirs(&self, unit: &SyncUnit) -> Vec<String> {
        let dir = unit_backups_dir(self.root(), &unit.key());
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = read
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Content of `rel` in the newest backup folder ending in `-<side>`.
    pub(crate) fn backup_of(&self, unit: &SyncUnit, side: &str, rel: &str) -> Option<Vec<u8>> {
        let suffix = format!("-{side}");
        let dir = self
            .backup_dirs(unit)
            .into_iter()
            .rev()
            .find(|name| name.ends_with(&suffix) || name.contains(&format!("{suffix}-")))?;
        let root = unit_backups_dir(self.root(), &unit.key()).join(dir);
        std::fs::read(join_rel(&root, rel)).ok()
    }

    /// Whether the unit's staging folder is gone / empty.
    pub(crate) fn staging_empty(&self, unit: &SyncUnit) -> bool {
        let dir = join_rel(&self.root().join("cloud").join("staging"), &unit.key());
        std::fs::read_dir(dir).map_or(true, |mut read| read.next().is_none())
    }
}

/// A Local folder provider in a fresh temp dir.
pub(crate) fn local_folder() -> (
    TempDir,
    crate::providers::local_folder_provider::LocalFolderProvider,
) {
    let dir = tempfile::tempdir().unwrap();
    let provider = crate::providers::local_folder_provider::LocalFolderProvider::new(dir.path());
    (dir, provider)
}
