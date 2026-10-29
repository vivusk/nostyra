//! Batch upsert, sha1 relink, missing marks and the rescan cache.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, scanned};
use storage_db::{
    file_fingerprints, find_game_id_by_rom_path, game_play_stats, get_game, mark_missing_paths,
    record_play_session, upsert_scanned_batch, NewPlaySession, RelinkedGame, UpsertOutcome,
};

#[test]
fn upsert_is_idempotent() {
    let (mut conn, folder) = db_with_folder();
    let batch = vec![
        scanned(folder, "a.sfc", "AAA", "Alpha", true),
        scanned(folder, "b.smc", "BBB", "Beta", false),
    ];
    let first = upsert_scanned_batch(&mut conn, &batch, 1).unwrap();
    assert_eq!(
        first,
        UpsertOutcome {
            inserted: 2,
            updated: 0,
            relinked: Vec::new()
        }
    );
    let second = upsert_scanned_batch(&mut conn, &batch, 2).unwrap();
    assert_eq!(
        second,
        UpsertOutcome {
            inserted: 0,
            updated: 2,
            relinked: Vec::new()
        }
    );

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
    // added_at is kept from the first insert.
    let id = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/a.sfc")
        .unwrap()
        .unwrap();
    assert_eq!(get_game(&conn, id).unwrap().unwrap().added_at, 1);
}

#[test]
fn update_refreshes_changed_file_data() {
    let (mut conn, folder) = db_with_folder();
    upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "a.sfc", "AAA", "Alpha", false)],
        1,
    )
    .unwrap();
    let mut changed = scanned(folder, "a.sfc", "CCC", "Alpha Fixed", true);
    changed.file_size = 2_097_152;
    upsert_scanned_batch(&mut conn, &[changed], 2).unwrap();

    let id = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/a.sfc")
        .unwrap()
        .unwrap();
    let game = get_game(&conn, id).unwrap().unwrap();
    assert_eq!(
        (game.sha1.as_str(), game.title.as_str(), game.file_size),
        ("CCC", "Alpha Fixed", 2_097_152)
    );
    assert!(!game.needs_review());
}

#[test]
fn moved_file_relinks_by_sha1_and_keeps_history() {
    let (mut conn, folder) = db_with_folder();
    upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "old.sfc", "AAA", "Alpha", true)],
        1,
    )
    .unwrap();
    let id = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/old.sfc")
        .unwrap()
        .unwrap();
    record_play_session(&conn, &session(id, 600)).unwrap();

    // Scanner order: vanished paths first, then new files.
    assert_eq!(
        mark_missing_paths(&mut conn, &["E:/ROMs/SNES/old.sfc".into()]).unwrap(),
        1
    );
    let outcome = upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "new.sfc", "AAA", "Alpha", true)],
        2,
    )
    .unwrap();
    assert_eq!(
        outcome,
        UpsertOutcome {
            inserted: 0,
            updated: 0,
            relinked: vec![RelinkedGame {
                game_id: id,
                platform_id: "snes".into(),
                old_rom_path: "E:/ROMs/SNES/old.sfc".into(),
                new_rom_path: "E:/ROMs/SNES/new.sfc".into(),
            }]
        }
    );

    let game = get_game(&conn, id).unwrap().unwrap();
    assert_eq!(game.rom_path, "E:/ROMs/SNES/new.sfc");
    assert!(!game.missing);
    assert_eq!(game_play_stats(&conn, id).unwrap().total_play_s, 600);
}

#[test]
fn present_duplicate_sha1_is_not_relinked() {
    let (mut conn, folder) = db_with_folder();
    upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "a.sfc", "AAA", "Alpha", true)],
        1,
    )
    .unwrap();
    // A second copy while the first is still present → its own row.
    let outcome = upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "copy.sfc", "AAA", "Alpha", true)],
        2,
    )
    .unwrap();
    assert_eq!(outcome.inserted, 1);
}

#[test]
fn fingerprints_cover_present_files_only() {
    let (mut conn, folder) = db_with_folder();
    let batch = vec![
        scanned(folder, "a.sfc", "AAA", "Alpha", true),
        scanned(folder, "b.sfc", "BBB", "Beta", true),
    ];
    upsert_scanned_batch(&mut conn, &batch, 1).unwrap();
    mark_missing_paths(&mut conn, &["E:/ROMs/SNES/b.sfc".into()]).unwrap();

    let cache = file_fingerprints(&conn, folder).unwrap();
    assert_eq!(cache.len(), 1);
    let fp = cache["E:/ROMs/SNES/a.sfc"];
    assert_eq!(
        (fp.size, fp.mtime_ns),
        (1_048_576, 1_700_000_000_000_000_000)
    );
}

#[test]
fn missing_file_reappearing_is_restored() {
    let (mut conn, folder) = db_with_folder();
    let game = scanned(folder, "a.sfc", "AAA", "Alpha", true);
    upsert_scanned_batch(&mut conn, std::slice::from_ref(&game), 1).unwrap();
    mark_missing_paths(&mut conn, std::slice::from_ref(&game.rom_path)).unwrap();
    upsert_scanned_batch(&mut conn, &[game], 2).unwrap();
    let id = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/a.sfc")
        .unwrap()
        .unwrap();
    assert!(!get_game(&conn, id).unwrap().unwrap().missing);
}

fn session(game_id: i64, duration_s: i64) -> NewPlaySession {
    NewPlaySession {
        game_id,
        started_at: 1_000,
        duration_s,
        exit_code: Some(0),
        source_key: None,
    }
}

#[test]
fn several_missing_candidates_relink_the_lowest_id() {
    let (mut conn, folder) = db_with_folder();
    let copies = [
        scanned(folder, "copy1.sfc", "DUP", "Dup", true),
        scanned(folder, "copy2.sfc", "DUP", "Dup", true),
    ];
    upsert_scanned_batch(&mut conn, &copies, 1).unwrap();
    let paths = [
        "E:/ROMs/SNES/copy1.sfc".into(),
        "E:/ROMs/SNES/copy2.sfc".into(),
    ];
    mark_missing_paths(&mut conn, &paths).unwrap();
    let outcome = upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "moved.sfc", "DUP", "Dup", true)],
        2,
    )
    .unwrap();
    let lowest = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/copy2.sfc")
        .unwrap()
        .unwrap()
        - 1;
    assert_eq!(outcome.relinked.len(), 1);
    assert_eq!(outcome.relinked[0].game_id, lowest);
    assert_eq!(outcome.relinked[0].old_rom_path, "E:/ROMs/SNES/copy1.sfc");
}
