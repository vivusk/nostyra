//! Play-time recording rule: runner record vs launcher measurement.

use super::*;

fn outcome(started_at: Option<i64>) -> SessionOutcome {
    SessionOutcome {
        exit_code: Some(0),
        started_at,
        measured_seconds: 90,
        error: None,
    }
}

fn imported(recorded: Vec<(i64, i64)>) -> PendingImportSummary {
    PendingImportSummary {
        recorded,
        ..PendingImportSummary::default()
    }
}

#[test]
fn runner_record_wins_over_measurement() {
    assert_eq!(
        measured_fallback(1, &outcome(Some(1_000)), &imported(vec![(1, 1_001)])),
        None
    );
}

#[test]
fn measurement_used_when_runner_record_missing_even_with_leftovers() {
    let leftover = imported(vec![(1, 10), (2, 1_000)]); // old file, other game
    let measured = measured_fallback(1, &outcome(Some(1_000)), &leftover).unwrap();
    assert_eq!((measured.started_at, measured.duration_s), (1_000, 90));
    assert_eq!(measured.source_key, None);
}

#[test]
fn never_started_session_records_nothing() {
    assert_eq!(
        measured_fallback(1, &outcome(None), &imported(vec![])),
        None
    );
}
