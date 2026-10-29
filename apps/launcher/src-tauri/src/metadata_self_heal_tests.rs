//! Self-heal: fills missing rows, rewrites stale non-edited rows, partial indexes,
//! and per-system lookups (re-headered NES rows find metadata via `dat_name`).

use super::*;
use storage_db::{
    add_library_folder, get_game_metadata, open_in_memory_database, record_play_session,
    set_setting, update_user_metadata, upsert_scanned_batch, NewPlaySession, ScannedGame,
    UserMetadataEdit,
};

fn game(folder_id: i64, file: &str, crc: &str, matched: bool) -> ScannedGame {
    ScannedGame {
        platform_id: "snes".into(),
        folder_id,
        rom_path: format!("roms/{file}"),
        file_size: 1,
        file_mtime_ns: 0,
        crc32: crc.into(),
        sha1: format!("S{crc}"),
        title: file.into(),
        region: None,
        dat_name: matched.then(|| file.to_string()),
        metadata: None,
        serial: None,
    }
}

fn genre_index(work: &MetadataWorkItem) -> Option<GameMetadataRecord> {
    let crc = work.crc32.as_str();
    (crc == "AAAA0001" || crc == "AAAA0002").then(|| GameMetadataRecord {
        genre: Some("Action".into()),
        ..GameMetadataRecord::default()
    })
}

/// M1-style library (no metadata rows) with play history.
fn m1_library() -> Connection {
    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "roms", 1).unwrap().id;
    let games = [
        game(folder, "a.sfc", "AAAA0001", true),
        game(folder, "b.sfc", "AAAA0002", true),
        game(folder, "c.sfc", "AAAA0003", false),
    ];
    upsert_scanned_batch(&mut conn, &games, 1).unwrap();
    let session = NewPlaySession {
        game_id: 1,
        started_at: 5,
        duration_s: 60,
        exit_code: Some(0),
        source_key: None,
    };
    record_play_session(&conn, &session).unwrap();
    conn
}

#[test]
fn fills_matched_games_once_and_keeps_play_history() {
    let mut conn = m1_library();
    assert_eq!(heal_metadata(&mut conn, &genre_index, true, 9).unwrap(), 2);
    let a = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(a.genre.as_deref(), Some("Action"));
    assert!(get_game_metadata(&conn, 3).unwrap().is_none(), "unmatched");
    assert_eq!(
        storage_db::game_play_stats(&conn, 1).unwrap().total_play_s,
        60
    );
    // Second run: nothing missing, version recorded → no-op.
    assert_eq!(heal_metadata(&mut conn, &genre_index, true, 10).unwrap(), 0);
}

#[test]
fn version_change_rewrites_non_edited_rows_only() {
    let mut conn = m1_library();
    heal_metadata(&mut conn, &genre_index, true, 9).unwrap();
    let edit = UserMetadataEdit {
        genre: Some("Puzzle".into()),
        ..UserMetadataEdit::default()
    };
    update_user_metadata(&conn, 2, &edit, "old", 9).unwrap();
    conn.execute("UPDATE game_metadata SET source_version = 'old'", [])
        .unwrap();
    set_setting(&conn, METADATA_VERSION_SETTING, "old").unwrap();
    assert_eq!(heal_metadata(&mut conn, &genre_index, true, 10).unwrap(), 1);
    let edited = get_game_metadata(&conn, 2).unwrap().unwrap();
    assert_eq!(edited.genre.as_deref(), Some("Puzzle"));
    assert!(edited.user_edited);
}

#[test]
fn rows_from_a_partial_index_are_re_healed_by_the_next_full_load() {
    let mut conn = m1_library();
    let no_genre = |work: &MetadataWorkItem| {
        genre_index(work).map(|_| GameMetadataRecord {
            developer: Some("Dev".into()),
            ..GameMetadataRecord::default()
        })
    };
    heal_metadata(&mut conn, &no_genre, false, 9).unwrap();
    let recorded = storage_db::get_setting(&conn, METADATA_VERSION_SETTING).unwrap();
    assert_eq!(recorded.as_deref(), Some(METADATA_PARTIAL_VERSION));
    assert_eq!(get_game_metadata(&conn, 1).unwrap().unwrap().genre, None);

    assert_eq!(heal_metadata(&mut conn, &genre_index, true, 10).unwrap(), 2);
    let healed = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(healed.genre.as_deref(), Some("Action"));
    let recorded = storage_db::get_setting(&conn, METADATA_VERSION_SETTING).unwrap();
    assert_eq!(recorded.as_deref(), Some(METADATA_SOURCE_VERSION));
}

#[test]
fn reheadered_nes_row_keeps_metadata_through_a_stale_heal() {
    use game_library::{parse_clrmamepro_dat, DatIndex, MetadataIndex, SystemCatalog};
    use system_catalog::NES;

    // DAT laid out like libretro-database: `.nes` and `.unh` in separate blocks.
    let name = "Fake Bros. (World)";
    let dat = format!(
        "game ( name \"{name}\" rom ( name \"{name}.nes\" size 24592 crc AAAA0001 ) )\n\
         game ( name \"{name}\" rom ( name \"{name}.unh\" size 24576 crc CCCC0003 ) )"
    );
    let temp = tempfile::Builder::new()
        .prefix("nostyra-heal-nes-")
        .tempdir()
        .unwrap();
    let genre_dir = temp.path().join("genre");
    std::fs::create_dir_all(&genre_dir).unwrap();
    std::fs::write(
        genre_dir.join(NES.metadat_file),
        format!("game ( comment \"{name}\" genre \"Platform\" rom ( crc AAAA0001 ) )"),
    )
    .unwrap();
    let catalogs = game_library::SystemCatalogs::from_loaded([(
        "nes",
        SystemCatalog {
            dat: DatIndex::from_entries(parse_clrmamepro_dat(&dat).unwrap()),
            metadata: MetadataIndex::load(temp.path(), NES.metadat_file),
        },
    )]);

    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "roms", 1).unwrap().id;
    // Re-headered dump: the stored whole-file CRC is in neither DAT nor metadat.
    let row = ScannedGame {
        platform_id: "nes".into(),
        dat_name: Some(name.into()),
        ..game(folder, "fake.nes", "BBBB0002", true)
    };
    upsert_scanned_batch(&mut conn, &[row], 1).unwrap();
    set_setting(&conn, METADATA_VERSION_SETTING, "old").unwrap();

    let lookup = |work: &MetadataWorkItem| {
        catalogs
            .metadata_for_game(
                &work.platform_id,
                work.dat_name.as_deref(),
                &work.crc32,
                work.serial.as_deref(),
            )
            .map(|(record, _)| record.clone())
    };
    assert_eq!(heal_metadata(&mut conn, &lookup, true, 9).unwrap(), 1);
    let healed = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(healed.genre.as_deref(), Some("Platform"));
}

/// PS1 rows match serial-keyed metadat: a stale heal must re-apply (never delete)
/// the metadata of a serial-matched translation whose CRC the metadat cannot know.
#[test]
fn ps1_rows_heal_by_serial_on_the_vendored_data() {
    let vendored = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../third-party/libretro-database");
    let catalogs = game_library::SystemCatalogs::new(Some(vendored));
    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "roms", 1).unwrap().id;
    let row = ScannedGame {
        platform_id: system_catalog::PSX.id.into(),
        dat_name: Some("CTR - Crash Team Racing (USA)".into()),
        serial: Some("SCUS-94426".into()),
        ..game(folder, "CTR VN.bin", "DEADBEEF", true)
    };
    upsert_scanned_batch(&mut conn, &[row], 1).unwrap();
    set_setting(&conn, METADATA_VERSION_SETTING, "old").unwrap();
    let lookup = |work: &MetadataWorkItem| {
        catalogs
            .metadata_for_game(
                &work.platform_id,
                work.dat_name.as_deref(),
                &work.crc32,
                work.serial.as_deref(),
            )
            .map(|(record, _)| record.clone())
    };
    assert_eq!(heal_metadata(&mut conn, &lookup, true, 9).unwrap(), 1);
    let healed = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(healed.developer.as_deref(), Some("Naughty Dog"));
}
