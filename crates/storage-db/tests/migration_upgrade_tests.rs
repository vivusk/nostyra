//! An M1 database (migration 001 only, with data) and an M9 database (up to 006)
//! upgrade to the latest schema keeping their rows.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use storage_db::{game_play_stats, get_game, list_games, open_database, GameListQuery};

#[test]
fn m1_database_upgrades_and_keeps_games_and_play_history() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-upgrade-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("nostyra.db");
    {
        // What an M1 launcher left behind: schema 001 + a played game.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../migrations/001_initial_schema.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch(
            "INSERT INTO library_folders (id, path, added_at) VALUES (1, 'E:/ROMs', 1);
             INSERT INTO games (id, platform_id, folder_id, rom_path, file_size, file_mtime_ns,
                crc32, sha1, title, title_search, region, dat_name, added_at)
             VALUES (1, 'snes', 1, 'E:/ROMs/ct.sfc', 1, 0, 'AAAA', 'BBBB', 'Chrono Trigger',
                'chrono trigger', 'USA', 'Chrono Trigger (USA)', 1);
             INSERT INTO play_sessions (game_id, started_at, duration_s) VALUES (1, 50, 3600);",
        )
        .unwrap();
    }
    let conn = open_database(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11, "001 + every later migration (latest 011)");
    let sega: Vec<(String, String)> = conn
        .prepare(
            "SELECT id, core_id FROM platforms WHERE core_id = 'genesis_plus_gx' ORDER BY rowid",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let sega_ids: Vec<&str> = sega.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(sega_ids, ["genesis", "sms", "gg", "sg1000"], "006 rows");
    let game = get_game(&conn, 1).unwrap().unwrap();
    assert_eq!(game.title, "Chrono Trigger");
    let serial: Option<String> = conn
        .query_row("SELECT serial FROM games WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(serial, None, "cartridge rows get a NULL serial");
    assert!(!game.favorite);
    assert_eq!((game.genre, game.cover_path), (None, None));
    assert_eq!(game_play_stats(&conn, 1).unwrap().total_play_s, 3600);
    assert_eq!(
        list_games(&conn, &GameListQuery::default()).unwrap().len(),
        1
    );
}

#[test]
fn m9_database_upgrades_to_the_n64_dreamcast_psp_and_nds_platforms_and_keeps_games() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-upgrade-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("nostyra.db");
    {
        // What an M9 launcher left behind: schema 001..006 + a Mega Drive game.
        let conn = rusqlite::Connection::open(&path).unwrap();
        for sql in [
            include_str!("../migrations/001_initial_schema.sql"),
            include_str!("../migrations/002_metadata_artwork_favorites.sql"),
            include_str!("../migrations/003_nes_platform.sql"),
            include_str!("../migrations/004_handheld_platforms.sql"),
            include_str!("../migrations/005_playstation_platform.sql"),
            include_str!("../migrations/006_sega_platforms.sql"),
        ] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 6).unwrap();
        conn.execute_batch(
            "INSERT INTO library_folders (id, path, added_at) VALUES (1, 'E:/ROMs', 1);
             INSERT INTO games (id, platform_id, folder_id, rom_path, file_size, file_mtime_ns,
                crc32, sha1, title, title_search, region, dat_name, added_at)
             VALUES (1, 'genesis', 1, 'E:/ROMs/s.md', 1, 0, 'AAAA', 'BBBB', 'Sonic',
                'sonic', 'World', 'Sonic (World)', 1);",
        )
        .unwrap();
    }
    let conn = open_database(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    let n64: (String, String, String, String) = conn
        .query_row(
            "SELECT name, extensions, dat_file, core_id FROM platforms WHERE id = 'n64'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        n64,
        (
            "N64".into(),
            "z64,v64,n64".into(),
            "Nintendo - Nintendo 64.dat".into(),
            "mupen64plus-next".into()
        ),
        "007 row"
    );
    let dc: (String, String, String, String) = conn
        .query_row(
            "SELECT name, extensions, dat_file, core_id FROM platforms WHERE id = 'dreamcast'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let want = (
        "DC",
        "gdi,cdi,chd,cue,m3u",
        "Sega - Dreamcast.dat",
        "flycast",
    );
    assert_eq!(
        (dc.0.as_str(), dc.1.as_str(), dc.2.as_str(), dc.3.as_str()),
        want,
        "008 row"
    );
    let psp: (String, String, String) = conn
        .query_row(
            "SELECT extensions, dat_file, core_id FROM platforms WHERE id = 'psp'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let want = (
        "iso,cso,chd,pbp",
        "Sony - PlayStation Portable.dat",
        "ppsspp",
    );
    assert_eq!(
        (psp.0.as_str(), psp.1.as_str(), psp.2.as_str()),
        want,
        "009 row"
    );
    let nds: (String, String, String, String) = conn
        .query_row(
            "SELECT name, extensions, dat_file, core_id FROM platforms WHERE id = 'nds'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let want = (
        "NDS",
        "nds,dsi,ids",
        "Nintendo - Nintendo DS.dat",
        "melonds_ds",
    );
    assert_eq!(
        (
            nds.0.as_str(),
            nds.1.as_str(),
            nds.2.as_str(),
            nds.3.as_str()
        ),
        want,
        "010 row"
    );
    assert_eq!(get_game(&conn, 1).unwrap().unwrap().title, "Sonic");
}
