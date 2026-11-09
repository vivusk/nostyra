//! Library-wide pass (launcher start, Settings → Sync now, "for all" conflict
//! choice): one `index.json` read finds units changed in the cloud, a stat-only scan
//! (hashes only for files whose size / mtime moved) finds units changed on this PC;
//! only those units are synced, one at a time. Conflicts are collected, not solved.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cloud_sync::{Choice, SyncOptions, SyncScope, SyncStatus, SyncUnit, SyncUnitError};

use crate::app_events::CLOUD_SYNC_PROGRESS;
use crate::cloud_change_detection::changed_or_dirty;
use crate::cloud_game_result::{error_kind, ErrorKind};
use crate::cloud_game_sync::{sync_units, CloudHost, UnitResult};
use crate::cloud_provider_factory::CloudAccount;
use crate::cloud_sync_state::JobKind;

/// How long a pass waits for a running job before giving up (next start retries).
const PASS_CLAIM_WAIT: Duration = Duration::from_secs(600);

/// One library game and its units.
#[derive(Debug, Clone)]
pub struct PassGame {
    pub game_id: i64,
    pub units: Vec<SyncUnit>,
}

/// What a pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PassSummary {
    pub synced_units: usize,
    /// Units left for later (offline, in use, stopped, failed).
    pub pending_units: usize,
    /// Games with a unit waiting for the user's choice.
    pub conflict_games: BTreeSet<i64>,
    /// Games whose units all synced (their old conflict / error marks go away).
    pub clean_games: BTreeSet<i64>,
    pub errors: BTreeMap<i64, String>,
    pub offline: bool,
    pub sign_in_expired: bool,
    /// The job slot could not be taken (another sync ran too long / closing).
    pub not_started: bool,
}

/// What to sync.
pub struct PassRequest {
    pub games: Vec<PassGame>,
    /// Applied to every conflict (bulk "This PC / Cloud for all").
    pub choice: Option<Choice>,
    /// `false`: sync every unit of `games` (no change detection).
    pub only_changed: bool,
}

/// Runs a pass inside its own job (blocking; call from a worker thread).
pub fn run_pass<H: CloudHost>(
    host: &H,
    account: &CloudAccount,
    scope: SyncScope,
    request: PassRequest,
) -> PassSummary {
    let ctx = host.ctx();
    let mut summary = PassSummary::default();
    // The change scan (index read + stat scan) runs before the claim: it only reads,
    // and a pre-play pull must not wait behind it (it cannot be cancelled).
    let units = unit_owners(&request.games);
    let candidates: Vec<_> = if request.only_changed {
        match changed_or_dirty(account, ctx.data_root, scope, &units) {
            Ok(keys) => units
                .into_iter()
                .filter(|(key, _)| keys.contains(key))
                .collect(),
            Err(err) => {
                tracing::warn!(target: "cloud_sync", error = %err, "cloud index unreadable");
                summary.offline = true;
                return summary;
            }
        }
    } else {
        units.into_iter().collect()
    };
    let stop = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + PASS_CLAIM_WAIT;
    let Some(guard) = ctx.cloud.claim(JobKind::Pass, deadline, stop) else {
        summary.not_started = true;
        return summary;
    };
    let options = match request.choice {
        Some(choice) => SyncOptions::both().with_choice(choice),
        None => SyncOptions::both(),
    };
    let total = candidates.len();
    let mut dirty_games: BTreeSet<i64> = BTreeSet::new();
    for (done, (_, (unit, games))) in candidates.into_iter().enumerate() {
        host.notify(
            CLOUD_SYNC_PROGRESS,
            serde_json::json!({ "done": done, "total": total }),
        );
        if summary.offline || !guard.yield_to_play() {
            summary.pending_units += 1;
            continue;
        }
        let result = sync_units(&ctx, &guard, account, scope, &[unit], options);
        match result.first() {
            Some(UnitResult::Done(outcome)) if outcome.status == SyncStatus::NeedsChoice => {
                summary.conflict_games.extend(&games);
                dirty_games.extend(&games);
            }
            Some(UnitResult::Done(_)) => summary.synced_units += 1,
            // Cancelled for a pre-play pull (not stopped): later units still go.
            Some(UnitResult::Failed(SyncUnitError::Cancelled)) if !guard.stopped() => {
                summary.pending_units += 1;
                dirty_games.extend(&games);
            }
            Some(UnitResult::Failed(err)) => {
                summary.pending_units += 1;
                dirty_games.extend(&games);
                match error_kind(err) {
                    ErrorKind::Offline => summary.offline = true,
                    ErrorKind::SignIn => summary.sign_in_expired = true,
                    ErrorKind::Other => games.iter().for_each(|game| {
                        summary.errors.insert(*game, err.to_string());
                    }),
                }
            }
            Some(UnitResult::InUse | UnitResult::Stopped) | None => {
                summary.pending_units += 1;
                dirty_games.extend(&games);
            }
        }
        if summary.sign_in_expired {
            summary.offline = true; // every other unit would fail the same way
        }
    }
    host.notify(
        CLOUD_SYNC_PROGRESS,
        serde_json::json!({ "done": total, "total": total }),
    );
    summary.clean_games = request
        .games
        .iter()
        .map(|game| game.game_id)
        .filter(|id| !dirty_games.contains(id))
        .collect();
    summary
}

/// Unit key → (unit, games using it); the shared PS2 unit appears once.
fn unit_owners(games: &[PassGame]) -> BTreeMap<String, (SyncUnit, Vec<i64>)> {
    let mut units: BTreeMap<String, (SyncUnit, Vec<i64>)> = BTreeMap::new();
    for game in games {
        for unit in &game.units {
            units
                .entry(unit.key())
                .or_insert_with(|| (unit.clone(), Vec::new()))
                .1
                .push(game.game_id);
        }
    }
    units
}
