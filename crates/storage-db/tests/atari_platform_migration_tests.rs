//! Migration 012: an M14 database (up to 011) gains the Atari 2600 / 7800 / Lynx
//! platform rows and keeps its games.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use storage_db::{get_game, open_database};

#[test]
fn m14_database_upgrades_to_the_atari_platforms() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-upgrade-012-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("nostyra.db");
    {
        // What an M14..M16 launcher left behind: schema 001..011 + a Wii game.
        let conn = rusqlite::Connection::open(&path).unwrap();
        for sql in [
            include_str!("../migrations/001_initial_schema.sql"),
            include_str!("../migrations/002_metadata_artwork_favorites.sql"),
            include_str!("../migrations/003_nes_platform.sql"),
            include_str!("../migrations/004_handheld_platforms.sql"),
            include_str!("../migrations/005_playstation_platform.sql"),
            include_str!("../migrations/006_sega_platforms.sql"),
            include_str!("../migrations/007_nintendo_64_platform.sql"),
            include_str!("../migrations/008_dreamcast_platform.sql"),
            include_str!("../migrations/009_psp_platform.sql"),
            include_str!("../migrations/010_nds_platform.sql"),
            include_str!("../migrations/011_gamecube_wii_platforms.sql"),
        ] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 11).unwrap();
        conn.execute_batch(
            "INSERT INTO library_folders (id, path, added_at) VALUES (1, 'E:/ROMs', 1);
             INSERT INTO games (id, platform_id, folder_id, rom_path, file_size, file_mtime_ns,
                crc32, sha1, title, title_search, region, dat_name, added_at)
             VALUES (1, 'wii', 1, 'E:/ROMs/w.rvz', 1, 0, 'AAAA', 'BBBB', 'Wii Sports',
                'wii sports', 'USA', 'Wii Sports (USA)', 1);",
        )
        .unwrap();
    }
    let conn = open_database(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 13);
    let rows: Vec<[String; 5]> = conn
        .prepare(
            "SELECT id, name, extensions, dat_file, core_id FROM platforms
             WHERE id IN ('atari2600', 'atari7800', 'lynx') ORDER BY rowid",
        )
        .unwrap()
        .query_map([], |r| {
            Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?])
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let want = [
        ["atari2600", "2600", "a26,bin", "Atari - 2600.dat", "stella"],
        [
            "atari7800",
            "7800",
            "a78,bin",
            "Atari - 7800.dat",
            "prosystem",
        ],
        ["lynx", "Lynx", "lnx,lyx,o,bll", "Atari - Lynx.dat", "handy"],
    ];
    assert_eq!(rows, want.map(|row| row.map(String::from)), "012 rows");
    assert_eq!(get_game(&conn, 1).unwrap().unwrap().title, "Wii Sports");
}
