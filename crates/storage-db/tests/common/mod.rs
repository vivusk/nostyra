//! Shared fixtures for storage-db integration tests.
#![allow(dead_code)] // each test crate uses a different subset
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use storage_db::{
    add_library_folder, find_game_id_by_rom_path, open_in_memory_database, record_play_session,
    Connection, GameFilter, GameListQuery, GameSort, NewPlaySession, ScannedGame,
};

/// Fresh in-memory database with one library folder; returns (conn, folder_id).
pub fn db_with_folder() -> (Connection, i64) {
    let conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "E:/ROMs/SNES", 100).unwrap();
    (conn, folder.id)
}

/// A scanned SNES file; `matched` decides whether it has a DAT name.
pub fn scanned(folder_id: i64, file: &str, sha1: &str, title: &str, matched: bool) -> ScannedGame {
    ScannedGame {
        platform_id: "snes".into(),
        folder_id,
        rom_path: format!("E:/ROMs/SNES/{file}"),
        file_size: 1_048_576,
        file_mtime_ns: 1_700_000_000_000_000_000,
        crc32: "B19ED489".into(),
        sha1: sha1.into(),
        title: title.into(),
        region: Some("USA".into()),
        dat_name: matched.then(|| format!("{title} (USA)")),
        metadata: None,
        serial: None,
    }
}

/// Records a finished session for the fixture file `file`.
pub fn play(conn: &Connection, file: &str, started_at: i64, duration_s: i64) {
    let game_id = find_game_id_by_rom_path(conn, &format!("E:/ROMs/SNES/{file}"))
        .unwrap()
        .unwrap();
    let session = NewPlaySession {
        game_id,
        started_at,
        duration_s,
        exit_code: Some(0),
        source_key: None,
    };
    record_play_session(conn, &session).unwrap();
}

/// Builds a list query.
pub fn list_query(search: Option<&str>, sort: GameSort, filter: GameFilter) -> GameListQuery {
    GameListQuery {
        search: search.map(Into::into),
        sort,
        filter,
        ..GameListQuery::default()
    }
}

/// Current unix time in seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
