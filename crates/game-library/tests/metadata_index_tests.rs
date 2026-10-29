//! Generalised parser on metadat files, MetadataIndex merge/partial loads, and scans
//! writing metadata for DAT-matched games.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::{Path, PathBuf};

use common::{fake_rom, TempLibrary};
use game_library::{hash_rom_bytes, parse_clrmamepro_games, DatEntry, DatIndex, MetadataIndex};
use storage_db::get_game_metadata;

const FILE: &str = "Nintendo - Super Nintendo Entertainment System.dat";

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/metadat")
}

#[test]
fn parser_reads_attribute_games_without_name_or_size() {
    let text = r#"
clrmamepro ( name "x" )
game (
	comment "Super Mario World (USA)"
	users 2
	serial "SNS-MW"
	rom ( crc b19ed489 )
)"#;
    let games = parse_clrmamepro_games(text).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].attr("comment"), Some("Super Mario World (USA)"));
    assert_eq!(games[0].attr("users"), Some("2"));
    assert_eq!(games[0].attr("name"), None);
    assert_eq!(games[0].roms[0].crc32.as_deref(), Some("B19ED489"));
    assert_eq!(games[0].roms[0].size, None);
    // PS1 metadat ROMs carry only serial + name: crc is optional for attribute
    // files, still required by the identification view.
    let crcless = parse_clrmamepro_games("game ( rom ( size 1 ) )").unwrap();
    assert_eq!(crcless[0].roms[0].crc32, None);
    assert!(
        game_library::parse_clrmamepro_dat("game ( name \"x\" rom ( size 1 ) )").is_err(),
        "crc required for identification"
    );
}

#[test]
fn index_merges_the_five_attribute_files_by_crc() {
    let index = MetadataIndex::load(&fixture_dir(), system_catalog::SNES.metadat_file);
    assert!(index.is_complete());
    assert_eq!(index.len(), 2);
    let chrono = index.lookup("2d5ce2ba").unwrap();
    assert_eq!(chrono.developer.as_deref(), Some("Square"));
    assert_eq!(chrono.publisher.as_deref(), Some("Squaresoft"));
    assert_eq!(chrono.genre.as_deref(), Some("Role-playing (RPG)"));
    assert_eq!((chrono.release_year, chrono.players), (Some(1995), Some(1)));
    let mario = index.lookup("B19ED489").unwrap();
    assert_eq!(
        mario.genre.as_deref(),
        Some("Platform"),
        "lower-case crc in file"
    );
    assert_eq!(
        (mario.release_year, mario.publisher.as_deref()),
        (None, None)
    );
    assert!(index.lookup("00000000").is_none());
}

/// Temp metadat dir with only the given attribute files (contents from `rows`).
fn temp_metadat(name: &str, files: &[(&str, String)]) -> tempfile::TempDir {
    let prefix = format!("nostyra-metadat-{name}-");
    let temp = tempfile::Builder::new().prefix(&prefix).tempdir().unwrap();
    for (attr, text) in files {
        std::fs::create_dir_all(temp.path().join(attr)).unwrap();
        std::fs::write(temp.path().join(attr).join(FILE), text).unwrap();
    }
    temp
}

#[test]
fn missing_or_corrupt_files_give_a_partial_index() {
    let genre = std::fs::read_to_string(fixture_dir().join("genre").join(FILE)).unwrap();
    let temp = temp_metadat(
        "partial",
        &[("genre", genre), ("developer", "game ( comment ".into())],
    );
    let dir = temp.path();
    let index = MetadataIndex::load(dir, system_catalog::SNES.metadat_file);
    assert!(!index.is_complete());
    assert_eq!(index.len(), 2);
    assert_eq!(index.lookup("2D5CE2BA").unwrap().developer, None);

    let empty = MetadataIndex::load(&dir.join("nope"), system_catalog::SNES.metadat_file);
    assert!(empty.is_empty() && !empty.is_complete());
}

#[test]
fn scan_writes_metadata_for_matched_games_only() {
    let lib = TempLibrary::new("metadata");
    let known = fake_rom(64 * 1024, 11);
    lib.write("known.sfc", &known);
    lib.write("unknown.sfc", &fake_rom(64 * 1024, 12));
    let hashes = hash_rom_bytes(&known, &system_catalog::SNES);
    let dat = DatIndex::from_entries(vec![DatEntry {
        name: "Known Quest (USA)".into(),
        region: Some("USA".into()),
        size: hashes.size,
        crc32: hashes.crc32.clone(),
        metadata_crc32: hashes.crc32.clone(),
        serial: None,
        sha1: Some(hashes.sha1.clone()),
    }]);
    let genre = format!(
        "game ( comment \"Known Quest (USA)\" genre \"Action\" rom ( crc {} ) )",
        hashes.crc32
    );
    let temp = temp_metadat("scan", &[("genre", genre)]);
    let metadata = MetadataIndex::load(temp.path(), system_catalog::SNES.metadat_file);

    let summary = lib.scan_with(&dat, &metadata);
    assert_eq!((summary.matched, summary.unmatched), (1, 1));
    let conn = lib.db.lock().unwrap();
    let known_id: i64 = conn
        .query_row("SELECT id FROM games WHERE dat_name IS NOT NULL", [], |r| {
            r.get(0)
        })
        .unwrap();
    let meta = get_game_metadata(&conn, known_id).unwrap().unwrap();
    assert_eq!(meta.genre.as_deref(), Some("Action"));
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM game_metadata", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "unmatched game has no metadata row");
}
