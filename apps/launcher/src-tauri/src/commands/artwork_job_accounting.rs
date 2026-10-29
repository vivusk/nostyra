//! Pure bookkeeping of the artwork job: which outcomes are stored, the finish counts,
//! and the transient circuit breaker that ends a job on a dead network.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use artwork_cache::{ArtworkKind, ArtworkOutcome, KindOutcome, THUMBNAIL_SOURCE};
use storage_db::{ArtworkRecord, ArtworkStatus};

use crate::app_events::ArtworkFinishedPayload;

/// Consecutive all-transient games after which the job stops itself.
pub const ARTWORK_TRANSIENT_BREAK: u32 = 5;

/// Status to store for an outcome; `None` = nothing is written (transient failures
/// stay absent so the next job retries them; cancelled kinds were never decided).
fn stored_status(outcome: &KindOutcome) -> Option<ArtworkStatus> {
    match outcome {
        KindOutcome::Cached { .. } => Some(ArtworkStatus::Cached),
        KindOutcome::NotFound => Some(ArtworkStatus::NotFound),
        KindOutcome::Failed(_) => Some(ArtworkStatus::Failed),
        KindOutcome::Transient(_) | KindOutcome::Cancelled => None,
    }
}

/// The row to store for one kind's outcome (`None` = store nothing).
pub fn artwork_record(
    game_id: i64,
    kind: ArtworkKind,
    outcome: &KindOutcome,
    checked_at: i64,
) -> Option<ArtworkRecord> {
    let status = stored_status(outcome)?;
    let (local_path, source_name) = match outcome {
        KindOutcome::Cached {
            relative_path,
            source_name,
        } => (Some(relative_path.clone()), Some(source_name.clone())),
        _ => (None, None),
    };
    Some(ArtworkRecord {
        game_id,
        kind: kind.as_str().to_string(),
        status,
        local_path,
        source: THUMBNAIL_SOURCE.to_string(),
        source_name,
        checked_at,
    })
}

/// Adds one game's per-kind outcomes to the finish counts.
pub fn count_outcome(totals: &mut ArtworkFinishedPayload, outcome: &ArtworkOutcome) {
    for (_, kind_outcome) in &outcome.results {
        match kind_outcome {
            KindOutcome::Cached { .. } => totals.cached += 1,
            KindOutcome::NotFound => totals.not_found += 1,
            KindOutcome::Failed(_) => totals.failed += 1,
            KindOutcome::Transient(_) => totals.transient += 1,
            KindOutcome::Cancelled => {}
        }
    }
}

/// Shared by all workers of a job. Counts consecutive games whose every kind was
/// transient; only an outcome that completed a network round-trip resets it, so
/// index-only "not found" results cannot hide a dead network.
#[derive(Debug, Default)]
pub struct TransientBreaker {
    consecutive: AtomicU32,
    tripped: AtomicBool,
}

impl TransientBreaker {
    /// Records one game; returns true when this game trips the breaker.
    pub fn record(&self, outcome: &ArtworkOutcome) -> bool {
        if outcome.is_transient() {
            let count = self.consecutive.fetch_add(1, Ordering::Relaxed) + 1;
            return count >= ARTWORK_TRANSIENT_BREAK && !self.tripped.swap(true, Ordering::Relaxed);
        }
        if outcome.network {
            self.consecutive.store(0, Ordering::Relaxed);
        }
        false
    }

    /// The job was stopped by the breaker (not by the user).
    pub fn tripped(&self) -> bool {
        self.tripped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use artwork_cache::ArtworkKind;

    fn outcome(kind_outcome: KindOutcome, network: bool) -> ArtworkOutcome {
        ArtworkOutcome {
            results: vec![(ArtworkKind::Cover, kind_outcome)],
            network,
        }
    }

    fn transient() -> ArtworkOutcome {
        outcome(KindOutcome::Transient("offline".into()), false)
    }

    #[test]
    fn breaker_trips_once_after_five_consecutive_transient_games() {
        let breaker = TransientBreaker::default();
        for _ in 0..4 {
            assert!(!breaker.record(&transient()));
        }
        assert!(breaker.record(&transient()));
        assert!(!breaker.record(&transient()), "trips only once");
        assert!(breaker.tripped());
    }

    #[test]
    fn breaker_resets_only_on_network_outcomes() {
        let breaker = TransientBreaker::default();
        for _ in 0..4 {
            breaker.record(&transient());
        }
        // Index-only not-found (no network) does not reset the streak.
        breaker.record(&outcome(KindOutcome::NotFound, false));
        assert!(breaker.record(&transient()));

        let breaker = TransientBreaker::default();
        for _ in 0..4 {
            breaker.record(&transient());
        }
        breaker.record(&outcome(KindOutcome::NotFound, true));
        assert!(!breaker.record(&transient()));
        assert!(!breaker.tripped());
    }

    #[test]
    fn transient_and_cancelled_outcomes_store_nothing() {
        assert_eq!(stored_status(&KindOutcome::Transient("x".into())), None);
        assert_eq!(stored_status(&KindOutcome::Cancelled), None);
        assert_eq!(
            stored_status(&KindOutcome::NotFound),
            Some(ArtworkStatus::NotFound)
        );
        assert_eq!(
            stored_status(&KindOutcome::Failed("x".into())),
            Some(ArtworkStatus::Failed)
        );
    }

    #[test]
    fn counts_add_up_per_kind() {
        let mut totals = ArtworkFinishedPayload::default();
        let game = ArtworkOutcome {
            results: vec![
                (
                    ArtworkKind::Cover,
                    KindOutcome::Cached {
                        relative_path: "1-cover.jpg".into(),
                        source_name: "x".into(),
                    },
                ),
                (ArtworkKind::Hero, KindOutcome::Transient("t".into())),
            ],
            network: true,
        };
        count_outcome(&mut totals, &game);
        count_outcome(&mut totals, &outcome(KindOutcome::NotFound, true));
        assert_eq!(
            (totals.cached, totals.transient, totals.not_found),
            (1, 1, 1)
        );
    }
}
