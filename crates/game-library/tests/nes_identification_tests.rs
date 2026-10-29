//! NES identification against a DAT laid out like libretro-database's: the headered
//! `.nes` ROM and the headerless `.unh` ROM of a game sit in two separate `game`
//! blocks with the same name; metadat files key the game by the headered CRC.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{
    hash_headerless, hash_rom_bytes, parse_clrmamepro_dat, scan_library, SystemCatalogs,
};
use storage_db::{get_game_metadata, list_games, GameListQuery};
use system_catalog::{ALL_SYSTEMS, NES};

const GAME: &str = "Fake Bros. (World)";

/// iNES 1.0 dump: 16 KiB PRG + 8 KiB CHR of deterministic bytes.
fn ines_rom() -> Vec<u8> {
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    rom.extend((0..24_576u32).map(|i| (i * 31 % 253) as u8));
    rom
}

/// Writes `<dir>/<NES dat>` + `<dir>/metadat/genre/<NES dat>` for [`ines_rom`].
fn write_nes_database(dir: &Path) {
    let rom = ines_rom();
    let whole = hash_rom_bytes(&rom, &NES);
    let bare = hash_headerless(&rom, &NES).unwrap();
    let dat = format!(
        "clrmamepro ( name \"Nintendo - Nintendo Entertainment System\" )\n\
         game (\n\tname \"{GAME}\"\n\trom ( name \"{GAME}.nes\" size {} crc {} sha1 {} )\n)\n\
         game (\n\tname \"{GAME}\"\n\trom ( name \"{GAME}.unh\" size {} crc {} sha1 {} )\n)\n",
        whole.size, whole.crc32, whole.sha1, bare.size, bare.crc32, bare.sha1
    );
    std::fs::write(dir.join(NES.dat_file), dat).unwrap();
    let genre = format!(
        "game (\n\tcomment \"{GAME}\"\n\tgenre \"Platform\"\n\trom ( crc {} )\n)\n",
        whole.crc32
    );
    std::fs::create_dir_all(dir.join("metadat/genre")).unwrap();
    std::fs::write(dir.join("metadat/genre").join(NES.metadat_file), genre).unwrap();
}

fn temp_dir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("nostyra-gl-{tag}-"))
        .tempdir()
        .unwrap()
}

#[test]
fn unh_entries_carry_the_headered_metadata_crc() {
    let db = temp_dir("nesdat");
    write_nes_database(db.path());
    let text = std::fs::read_to_string(db.path().join(NES.dat_file)).unwrap();
    let entries = parse_clrmamepro_dat(&text).unwrap();
    assert_eq!(entries.len(), 2);
    let headered = hash_rom_bytes(&ines_rom(), &NES).crc32;
    assert!(entries.iter().all(|e| e.metadata_crc32 == headered));
}

#[test]
fn headered_and_reheadered_dumps_match_with_metadata() {
    let db = temp_dir("nesdb");
    write_nes_database(db.path());
    let lib = TempLibrary::new("nes-identify");
    lib.write("NES/fake.nes", &ines_rom());
    let mut reheadered = ines_rom();
    reheadered[7] = 0x08; // NES 2.0 rewrite of the same dump
    lib.write("Favorites/fake (nes2).nes", &reheadered);
    let mut unknown = ines_rom();
    unknown[100] ^= 0xFF;
    lib.write("homebrew.nes", &unknown);

    let catalogs = SystemCatalogs::new(Some(db.path().to_path_buf()));
    let summary = scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        &catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        (summary.added, summary.matched, summary.unmatched),
        (3, 2, 1)
    );
    assert!(
        !catalogs.is_loaded("snes"),
        "no SNES file → SNES catalog never loaded"
    );

    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    for game in &games {
        assert_eq!(game.platform_id, "nes");
        let metadata = get_game_metadata(&lib.db.lock().unwrap(), game.id).unwrap();
        if game.rom_path.ends_with("homebrew.nes") {
            assert_eq!(game.dat_name, None, "unknown dump needs review");
            assert!(metadata.is_none());
        } else {
            assert_eq!(game.dat_name.as_deref(), Some(GAME));
            assert_eq!(metadata.unwrap().genre.as_deref(), Some("Platform"));
        }
    }
    // Stored hashes are whole-file: the re-headered row keeps its own CRC.
    let nes2 = games
        .iter()
        .find(|g| g.rom_path.ends_with("(nes2).nes"))
        .unwrap();
    assert_eq!(nes2.crc32, hash_rom_bytes(&reheadered, &NES).crc32);

    // Self-heal / reset path: the stored CRC is unknown to the metadat, the DAT name
    // resolves the headered CRC.
    let (record, _) = catalogs
        .metadata_for_game("nes", Some(GAME), &nes2.crc32, None)
        .expect("metadata via dat_name");
    assert_eq!(record.genre.as_deref(), Some("Platform"));
    assert!(catalogs
        .metadata_for_game("nes", None, &nes2.crc32, None)
        .is_none());
}

#[test]
fn missing_files_give_empty_catalogs_and_dat_missing() {
    let empty = temp_dir("nodat");
    let catalogs = SystemCatalogs::new(Some(empty.path().to_path_buf()));
    assert!(catalogs.any_dat_missing());
    assert!(catalogs.get("nes").dat.is_empty());
    assert!(!catalogs.loaded_complete(), "missing metadat = partial");
    assert!(
        catalogs.get("no-such-system").dat.is_empty(),
        "unknown system"
    );
    assert!(SystemCatalogs::new(None).any_dat_missing());

    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    assert!(!catalogs.any_dat_missing());
    assert!(!catalogs.is_loaded("nes"), "file check loads nothing");
}

#[test]
fn vendored_nes_dat_parses() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let nes = catalogs.get("nes");
    assert!(nes.dat.len() > 10_000);
    assert!(nes.metadata.is_complete());
    assert_eq!(
        nes.dat.metadata_crc_for_name("Super Mario Bros. (World)"),
        Some("393A432F")
    );
    assert!(nes.metadata.lookup("393A432F").is_some());
}

#[test]
fn every_registry_system_has_a_platforms_row() {
    let conn = storage_db::open_in_memory_database().unwrap();
    for system in ALL_SYSTEMS {
        let (core, dat): (String, String) = conn
            .query_row(
                "SELECT core_id, dat_file FROM platforms WHERE id = ?1",
                [system.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|err| panic!("platforms row for {}: {err}", system.id));
        assert_eq!(
            (core.as_str(), dat.as_str()),
            (system.default_core_id, system.dat_file)
        );
    }
}

#[test]
fn update_scripts_list_every_registry_dat() {
    let scripts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts");
    for script in ["update-libretro-dat.sh", "update-libretro-dat.ps1"] {
        let text = std::fs::read_to_string(scripts.join(script)).unwrap();
        // Entries are "<source>/<name>.dat" (no-intro/…, redump/…).
        let listed: Vec<&str> = text
            .lines()
            .map(|line| line.trim().trim_matches(|c| c == '"' || c == '\''))
            .filter(|line| {
                (line.starts_with("no-intro/") || line.starts_with("redump/"))
                    && line.ends_with(".dat")
            })
            .filter_map(|line| line.split_once('/').map(|(_, name)| name))
            .collect();
        let registry: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.dat_file).collect();
        assert_eq!(listed, registry, "{script}");
        for system in ALL_SYSTEMS {
            assert_eq!(system.metadat_file, system.dat_file, "scripts use one name");
        }
    }
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    for system in ALL_SYSTEMS {
        for attr in ["developer", "publisher", "genre", "releaseyear", "maxusers"] {
            let file = vendored
                .join("metadat")
                .join(attr)
                .join(system.metadat_file);
            assert!(file.is_file(), "{}", file.display());
        }
    }
}

#[test]
fn duplicate_names_keep_their_own_metadata_crc() {
    let text = r#"
game ( name "Dup (World)" rom ( name "Dup (World).nes" size 1040 crc AAAAAAAA ) )
game ( name "Dup (World)" rom ( name "Dup (World).unh" size 1024 crc 11111111 ) )
game ( name "Dup (World)" rom ( name "Dup (World).nes" size 2064 crc BBBBBBBB ) )
game ( name "Dup (World)" rom ( name "Dup (World).unh" size 2048 crc 22222222 ) )
game ( name "Lost (World)" rom ( name "Lost (World).nes" size 1040 crc CCCCCCCC ) )
game ( name "Lost (World)" rom ( name "Lost (World).nes" size 1040 crc DDDDDDDD ) )
game ( name "Lost (World)" rom ( name "Lost (World).unh" size 4096 crc 33333333 ) )
"#;
    let entries = parse_clrmamepro_dat(text).unwrap();
    let meta: Vec<(&str, &str)> = entries
        .iter()
        .map(|e| (e.crc32.as_str(), e.metadata_crc32.as_str()))
        .collect();
    assert_eq!(
        meta,
        [
            ("AAAAAAAA", "AAAAAAAA"),
            ("11111111", "AAAAAAAA"),
            ("BBBBBBBB", "BBBBBBBB"),
            ("22222222", "BBBBBBBB"),
            ("CCCCCCCC", "CCCCCCCC"),
            ("DDDDDDDD", "DDDDDDDD"),
            ("33333333", "33333333"), // ambiguous twin: own CRC
        ]
    );
}

#[test]
fn one_absent_registry_dat_counts_as_missing() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let partial = temp_dir("onedat");
    std::fs::copy(
        vendored.join(ALL_SYSTEMS[0].dat_file),
        partial.path().join(ALL_SYSTEMS[0].dat_file),
    )
    .unwrap();
    assert!(SystemCatalogs::new(Some(partial.path().to_path_buf())).any_dat_missing());
}
