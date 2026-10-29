//! Migration 011: an M13 database (up to 010) gains the GameCube and Wii platform
//! rows (both on Dolphin) and keeps its games.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use storage_db::{get_game, open_database};

#[test]
fn m13_database_upgrades_to_the_gamecube_and_wii_platforms() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-upgrade-011-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("nostyra.db");
    {
        // What an M13 launcher left behind: schema 001..010 + a DS game.
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
        ] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 10).unwrap();
        conn.execute_batch(
            "INSERT INTO library_folders (id, path, added_at) VALUES (1, 'E:/ROMs', 1);
             INSERT INTO games (id, platform_id, folder_id, rom_path, file_size, file_mtime_ns,
                crc32, sha1, title, title_search, region, dat_name, added_at)
             VALUES (1, 'nds', 1, 'E:/ROMs/z.nds', 1, 0, 'AAAA', 'BBBB', 'Zelda',
                'zelda', 'USA', 'Zelda (USA)', 1);",
        )
        .unwrap();
    }
    let conn = open_database(&path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    let rows: Vec<(String, String, String, String, String)> = conn
        .prepare(
            "SELECT id, name, extensions, dat_file, core_id FROM platforms
             WHERE core_id = 'dolphin-emu' ORDER BY rowid",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let want = [
        (
            "gc",
            "GC",
            "iso,gcm,ciso,gcz,rvz,wia,dol,elf,m3u",
            "Nintendo - GameCube.dat",
        ),
        (
            "wii",
            "Wii",
            "iso,ciso,gcz,rvz,wia,wbfs,dol,elf",
            "Nintendo - Wii.dat",
        ),
    ];
    assert_eq!(rows.len(), want.len(), "011 rows");
    for (row, (id, name, exts, dat)) in rows.iter().zip(want) {
        assert_eq!(
            (
                row.0.as_str(),
                row.1.as_str(),
                row.2.as_str(),
                row.3.as_str()
            ),
            (id, name, exts, dat)
        );
    }
    assert_eq!(get_game(&conn, 1).unwrap().unwrap().title, "Zelda");
}
