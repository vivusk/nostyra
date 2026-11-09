//! Runs unit syncs inside a claimed job and turns their outcomes into what the UI
//! sees ([`SyncGameResult`]). No Tauri types: the launcher state is reached through
//! [`CloudHost`] (the app handle in production, a plain struct in tests).

use std::path::Path;
use std::sync::Mutex;

use cloud_sync::{
    sync_unit, BaseManifestStore, CloudError, DeviceIdentity, SyncContext, SyncOptions,
    SyncOutcome, SyncScope, SyncUnit, SyncUnitError,
};

use crate::app_state::ActiveSession;
use crate::cloud_job_guard::{JobGuard, UnitRefused};
use crate::cloud_provider_factory::CloudAccount;
use crate::cloud_sync_state::CloudSyncState;

/// The launcher state a cloud job needs.
pub struct CloudCtx<'a> {
    pub data_root: &'a Path,
    pub session: &'a Mutex<Option<ActiveSession>>,
    pub cloud: &'a CloudSyncState,
}

/// Owner of the launcher state that worker threads can carry (`AppHandle`).
pub trait CloudHost: Clone + Send + Sync + 'static {
    fn ctx(&self) -> CloudCtx<'_>;
    /// A status / progress event for the UI (no-op in tests).
    fn notify(&self, event: &str, payload: serde_json::Value);
}

/// What one unit sync ended with.
#[derive(Debug)]
pub enum UnitResult {
    Done(SyncOutcome),
    /// Held by a running game: skipped, synced after the session.
    InUse,
    /// The job was stopped before this unit.
    Stopped,
    Failed(SyncUnitError),
}

/// Syncs `units` one after the other inside `guard`'s job.
pub fn sync_units(
    ctx: &CloudCtx<'_>,
    guard: &JobGuard<'_>,
    account: &CloudAccount,
    scope: SyncScope,
    units: &[SyncUnit],
    options: SyncOptions,
) -> Vec<UnitResult> {
    let device = match DeviceIdentity::load_or_create(ctx.data_root) {
        Ok(device) => device,
        Err(err) => {
            tracing::warn!(target: "cloud_sync", "no device identity: {err}");
            let error = CloudError::Other(format!("device identity: {err}"));
            return units
                .iter()
                .map(|_| UnitResult::Failed(SyncUnitError::Cloud(error.clone())))
                .collect();
        }
    };
    let bases = BaseManifestStore::new(ctx.data_root, &account.account_key);
    let mut results = Vec::with_capacity(units.len());
    for unit in units {
        let key = unit.key();
        let progress = match guard.enter_unit(ctx.session, &key) {
            Ok(progress) => progress,
            Err(UnitRefused::InUse) => {
                tracing::info!(target: "cloud_sync", unit = %key, "skipped: in use by a running game");
                results.push(UnitResult::InUse);
                continue;
            }
            Err(UnitRefused::Stopped) => {
                results.push(UnitResult::Stopped);
                continue;
            }
        };
        let sync = SyncContext {
            provider: account.provider.as_ref(),
            data_root: ctx.data_root,
            bases: &bases,
            device: &device,
            scope,
            progress: &progress,
        };
        let result = sync_unit(&sync, unit, options);
        guard.leave_unit();
        results.push(match result {
            Ok(outcome) => UnitResult::Done(outcome),
            Err(err) => {
                tracing::warn!(target: "cloud_sync", unit = %key, error = %err, "unit not synced");
                UnitResult::Failed(err)
            }
        });
    }
    results
}
