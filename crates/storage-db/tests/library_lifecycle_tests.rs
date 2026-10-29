//! Folder removal/re-add, play sessions, cores and settings.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, scanned};
use storage_db::{
    add_library_folder, find_game_id_by_rom_path, game_play_stats, get_game, get_installed_core,
    get_setting, library_summary, list_games, list_library_folders, record_play_session,
    remove_library_folder, save_installed_core, set_setting, upsert_scanned_batch, GameListQuery,
    InstalledCoreRecord, NewPlaySession, PlayStats,
};

#[test]
fn removing_folder_hides_games_and_readding_restores_history() {
    let (mut conn, folder) = db_with_folder();
    let game = scanned(folder, "a.sfc", "AAA", "Alpha", true);
    upsert_scanned_batch(&mut conn, std::slice::from_ref(&game), 1).unwrap();
    let id = find_game_id_by_rom_path(&conn, &game.rom_path)
        .unwrap()
        .unwrap();
    record_play_session(&conn, &session(id, 120, None)).unwrap();

    assert!(remove_library_folder(&mut conn, folder).unwrap());
    assert!(!remove_library_folder(&mut conn, folder).unwrap());
    assert!(list_games(&conn, &GameListQuery::default())
        .unwrap()
        .is_empty());
    assert_eq!(library_summary(&conn).unwrap().total, 0);
    let hidden = get_game(&conn, id).unwrap().unwrap();
    assert_eq!((hidden.folder_id, hidden.missing), (None, true));
    assert_eq!(game_play_stats(&conn, id).unwrap().total_play_s, 120);

    // Re-add + rescan: matched by rom_path, same row, history intact.
    let readded = add_library_folder(&conn, "E:/ROMs/SNES", 5).unwrap();
    let rescanned = scanned(readded.id, "a.sfc", "AAA", "Alpha", true);
    upsert_scanned_batch(&mut conn, &[rescanned], 6).unwrap();
    let listed = list_games(&conn, &GameListQuery::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id, listed[0].total_play_s, listed[0].missing),
        (id, 120, false)
    );
}

#[test]
fn adding_same_folder_twice_is_idempotent() {
    let (conn, folder) = db_with_folder();
    let again = add_library_folder(&conn, "E:/ROMs/SNES", 999).unwrap();
    assert_eq!((again.id, again.added_at), (folder, 100));
    assert_eq!(list_library_folders(&conn).unwrap().len(), 1);
}

#[test]
fn play_stats_sum_sessions_and_source_key_dedupes() {
    let (mut conn, folder) = db_with_folder();
    upsert_scanned_batch(
        &mut conn,
        &[scanned(folder, "a.sfc", "AAA", "Alpha", true)],
        1,
    )
    .unwrap();
    let id = find_game_id_by_rom_path(&conn, "E:/ROMs/SNES/a.sfc")
        .unwrap()
        .unwrap();
    assert_eq!(game_play_stats(&conn, id).unwrap(), PlayStats::default());

    let pending = session(id, 300, Some("1700-42.json"));
    assert!(record_play_session(&conn, &pending).unwrap());
    assert!(!record_play_session(&conn, &pending).unwrap()); // second import ignored
    let mut later = session(id, 200, None);
    later.started_at = 5_000;
    assert!(record_play_session(&conn, &later).unwrap());
    // Two launcher-measured sessions (NULL source_key) are both kept.
    assert!(record_play_session(&conn, &later).unwrap());

    assert_eq!(
        game_play_stats(&conn, id).unwrap(),
        PlayStats {
            total_play_s: 700,
            last_played_at: Some(5_000),
            session_count: 3
        }
    );
}

#[test]
fn installed_core_roundtrip_and_replace() {
    let (conn, _) = db_with_folder();
    assert_eq!(get_installed_core(&conn, "snes9x").unwrap(), None);
    let mut core = InstalledCoreRecord {
        id: "snes9x".into(),
        platform_id: "snes".into(),
        library_path: "cores/snes9x_libretro.dll".into(),
        version: Some("1.63".into()),
        sha256: "AB".into(),
        installed_at: 10,
    };
    save_installed_core(&conn, &core).unwrap();
    core.sha256 = "CD".into();
    save_installed_core(&conn, &core).unwrap();
    assert_eq!(get_installed_core(&conn, "snes9x").unwrap(), Some(core));
}

#[test]
fn settings_roundtrip() {
    let (conn, _) = db_with_folder();
    assert_eq!(get_setting(&conn, "first_run_done").unwrap(), None);
    set_setting(&conn, "first_run_done", "1").unwrap();
    set_setting(&conn, "first_run_done", "2").unwrap();
    assert_eq!(
        get_setting(&conn, "first_run_done").unwrap().as_deref(),
        Some("2")
    );
}

fn session(game_id: i64, duration_s: i64, source_key: Option<&str>) -> NewPlaySession {
    NewPlaySession {
        game_id,
        started_at: 1_000,
        duration_s,
        exit_code: Some(0),
        source_key: source_key.map(Into::into),
    }
}
