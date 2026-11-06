//! Pending sessions written by the web edition (`<ms>-web.json`, `rom_path` =
//! `web:<folder>/<path>`, plus `content_sha1`): matched by sha1, kept while the game
//! is not scanned yet, dropped after the wait; runner records unchanged.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ipc_protocol::{pending_sessions_dir, PendingSessionRecord};
use runner_session::{import_pending_sessions, WEB_RECORD_WAIT_S};
use storage_db::{
    add_library_folder, find_game_id_by_rom_path, game_play_stats, open_in_memory_database,
    upsert_scanned_batch, Connection, ScannedGame,
};

const SHA1: &str = "6B47BB75D16514B6A476AA0C73A683A2A4C18765";
const DESKTOP_ROM: &str = "E:/ROMs/Chrono Trigger.sfc";

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("web-pending-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(pending_sessions_dir(&dir)).unwrap();
    dir
}

fn now() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap()
}

fn library(sha1: &str) -> Connection {
    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "E:/ROMs", 1).unwrap();
    let game = ScannedGame {
        platform_id: "snes".into(),
        folder_id: folder.id,
        rom_path: DESKTOP_ROM.into(),
        file_size: 4_194_304,
        file_mtime_ns: 0,
        crc32: "0".into(),
        sha1: sha1.into(),
        title: "Chrono Trigger".into(),
        region: None,
        dat_name: None,
        metadata: None,
        serial: None,
    };
    upsert_scanned_batch(&mut conn, &[game], 1).unwrap();
    conn
}

fn write_record(data: &Path, file: &str, started_at: i64, content_sha1: Option<&str>) -> PathBuf {
    let record = PendingSessionRecord {
        rom_path: "web:1a2b3c4d/SNES/Chrono Trigger.sfc".into(),
        started_at,
        play_seconds: 600,
        exit_code: 0,
        content_sha1: content_sha1.map(str::to_string),
    };
    let path = pending_sessions_dir(data).join(file);
    std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    path
}

fn game_id(conn: &Connection) -> i64 {
    find_game_id_by_rom_path(conn, DESKTOP_ROM)
        .unwrap()
        .unwrap()
}

#[test]
fn web_record_counts_for_the_game_with_that_sha1() {
    let data = scratch("match");
    let conn = library(SHA1);
    // Lower-case sha1 still matches (the web writes upper-case; any case is accepted).
    let file = write_record(
        &data,
        "1700000000000-web.json",
        now() - 60,
        Some(&SHA1.to_lowercase()),
    );
    let summary = import_pending_sessions(&conn, &data).unwrap();
    assert_eq!((summary.imported, summary.waiting), (1, 0));
    assert!(!file.exists(), "imported records are removed");
    let stats = game_play_stats(&conn, game_id(&conn)).unwrap();
    assert_eq!((stats.session_count, stats.total_play_s), (1, 600));
}

#[test]
fn unmatched_web_record_waits_then_is_dropped() {
    let data = scratch("wait");
    let conn = library("0000000000000000000000000000000000000000");
    let recent = write_record(&data, "2-web.json", now() - 3_600, Some(SHA1));
    let old = write_record(
        &data,
        "1-web.json",
        now() - WEB_RECORD_WAIT_S - 60,
        Some(SHA1),
    );
    let summary = import_pending_sessions(&conn, &data).unwrap();
    assert_eq!((summary.waiting, summary.unknown_game), (1, 1));
    assert!(recent.exists(), "kept until the desktop scans that ROM");
    assert!(!old.exists(), "dropped after the wait");

    // The ROM is scanned later: the kept record now counts.
    let conn = library(SHA1);
    assert_eq!(import_pending_sessions(&conn, &data).unwrap().imported, 1);
    assert!(!recent.exists());
}

#[test]
fn records_without_a_sha1_and_invalid_files_behave_as_before() {
    let data = scratch("old");
    let conn = library(SHA1);
    let runner = write_record(&data, "3-123.json", now(), None);
    let corrupt = pending_sessions_dir(&data).join("4-123.json");
    std::fs::write(&corrupt, b"{not json").unwrap();
    let in_progress = pending_sessions_dir(&data).join("5-web.json.crswap");
    std::fs::write(&in_progress, b"{}").unwrap();
    let summary = import_pending_sessions(&conn, &data).unwrap();
    assert_eq!(
        (
            summary.imported,
            summary.unknown_game,
            summary.invalid,
            summary.waiting
        ),
        (0, 1, 1, 0)
    );
    assert!(!runner.exists() && !corrupt.exists());
    assert!(
        in_progress.exists(),
        "a fresh swap file may still be written"
    );
    assert_eq!(
        game_play_stats(&conn, game_id(&conn))
            .unwrap()
            .session_count,
        0
    );
}
