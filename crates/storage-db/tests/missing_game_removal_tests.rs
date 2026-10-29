//! Missing filter, missing count, and removing missing games for good.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, list_query, play, scanned};
use storage_db::{
    delete_missing_game, find_game_id_by_rom_path, game_files_keys, game_play_stats, get_game,
    library_summary, list_games, mark_missing_paths, remove_library_folder, set_favorite,
    upsert_scanned_batch, visible_missing_game_ids, Connection, GameFilter, GameSort,
};

/// Folder with Alpha (present) and Beta (missing, played); returns (conn, folder, alpha, beta).
fn library_with_missing_beta() -> (Connection, i64, i64, i64) {
    let (mut conn, folder) = db_with_folder();
    let games = [
        scanned(folder, "a.sfc", "AAA", "Alpha", true),
        scanned(folder, "b.sfc", "BBB", "Beta", false),
    ];
    upsert_scanned_batch(&mut conn, &games, 1).unwrap();
    play(&conn, "b.sfc", 10, 60);
    mark_missing_paths(&mut conn, &["E:/ROMs/SNES/b.sfc".to_string()]).unwrap();
    let id = |file: &str| {
        find_game_id_by_rom_path(&conn, &format!("E:/ROMs/SNES/{file}"))
            .unwrap()
            .unwrap()
    };
    let (alpha, beta) = (id("a.sfc"), id("b.sfc"));
    (conn, folder, alpha, beta)
}

#[test]
fn missing_filter_and_summary_count_only_missing_games() {
    let (conn, _, _, beta) = library_with_missing_beta();
    let listed = list_games(
        &conn,
        &list_query(None, GameSort::Title, GameFilter::Missing),
    )
    .unwrap();
    assert_eq!(listed.iter().map(|g| g.id).collect::<Vec<_>>(), [beta]);
    assert_eq!(library_summary(&conn).unwrap().missing, 1);
    assert_eq!(visible_missing_game_ids(&conn).unwrap(), [beta]);
}

#[test]
fn deleting_a_missing_game_drops_its_row_and_history() {
    let (conn, _, alpha, beta) = library_with_missing_beta();
    set_favorite(&conn, beta, true).unwrap();

    assert!(delete_missing_game(&conn, beta).unwrap());
    assert!(get_game(&conn, beta).unwrap().is_none());
    assert_eq!(game_play_stats(&conn, beta).unwrap().total_play_s, 0);
    assert_eq!(library_summary(&conn).unwrap().missing, 0);
    // A present game is never removed; an unknown id is a no-op.
    assert!(!delete_missing_game(&conn, alpha).unwrap());
    assert!(get_game(&conn, alpha).unwrap().is_some());
    assert!(!delete_missing_game(&conn, beta).unwrap());
}

#[test]
fn hidden_rows_of_removed_folders_are_not_listed_as_missing() {
    let (mut conn, folder, _, _) = library_with_missing_beta();
    remove_library_folder(&mut conn, folder).unwrap();
    assert!(visible_missing_game_ids(&conn).unwrap().is_empty());
    assert_eq!(library_summary(&conn).unwrap().missing, 0);
}

#[test]
fn game_files_keys_list_path_and_sha1_per_platform() {
    let (conn, _, alpha, beta) = library_with_missing_beta();
    let keys = game_files_keys(&conn, Some("snes")).unwrap();
    let ids: Vec<i64> = keys.iter().map(|k| k.game_id).collect();
    assert_eq!(ids, [alpha, beta]);
    assert_eq!(keys[0].rom_path, "E:/ROMs/SNES/a.sfc");
    assert!(!keys[0].sha1.is_empty() && keys[0].platform_id == "snes");
    assert_eq!(game_files_keys(&conn, None).unwrap(), keys);
    assert!(game_files_keys(&conn, Some("nes")).unwrap().is_empty());
}
