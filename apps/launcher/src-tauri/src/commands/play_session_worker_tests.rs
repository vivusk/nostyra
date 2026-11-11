//! Play-time recording rule (runner record vs launcher measurement) and the
//! post-session table by netplay seat (F14).

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

#[test]
fn post_session_by_netplay_seat() {
    let table = [
        (None, true, true),
        (Some(SeatRole::Host), true, true),
        (Some(SeatRole::Player), false, true),
        (Some(SeatRole::Spectator), false, false),
    ];
    for (role, push, play_time) in table {
        assert_eq!(
            post_session(role),
            PostSession { push, play_time },
            "{role:?}"
        );
    }
}

#[test]
fn spectator_play_file_of_this_runner_only_is_dropped() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-spectator-")
        .tempdir()
        .unwrap();
    let dir = ipc_protocol::pending_sessions_dir(temp.path());
    std::fs::create_dir_all(&dir).unwrap();
    for name in [
        "1700000000000-42.json",
        "1700000000000-420.json",
        "1-7.json",
    ] {
        std::fs::write(dir.join(name), b"{}").unwrap();
    }
    discard_pending_of(temp.path(), 42);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["1-7.json", "1700000000000-420.json"]);
}
