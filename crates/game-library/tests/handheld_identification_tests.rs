//! Game Boy, Game Boy Color and Game Boy Advance identification: whole-file hashes
//! against a fixture DAT laid out like libretro-database's, metadata from a metadat
//! genre file, and files failing the header probe reported as scan failures.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{hash_rom_bytes, scan_library, SystemCatalogs};
use storage_db::{get_game_metadata, list_games, GameListQuery};
use system_catalog::{SystemSpec, GB, GBA, GBC};

/// 32 KiB GB/GBC image: deterministic bytes, the 4 logo bytes mGBA checks at 0x104.
fn gb_rom(seed: u32) -> Vec<u8> {
    let mut rom: Vec<u8> = (0..32 * 1024u32)
        .map(|i| (i.wrapping_mul(seed) % 251) as u8)
        .collect();
    rom[0x104..0x108].copy_from_slice(&[0xCE, 0xED, 0x66, 0x66]);
    rom
}

/// 64 KiB GBA image starting with an ARM branch (byte 3 = 0xEA).
fn gba_rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..64 * 1024u32).map(|i| (i * 17 % 253) as u8).collect();
    rom[..4].copy_from_slice(&[0x2E, 0x00, 0x00, 0xEA]);
    rom
}

/// Writes `<dir>/<dat>` + `<dir>/metadat/genre/<dat>` naming `rom` as `game`.
fn write_database(dir: &Path, spec: &SystemSpec, game: &str, ext: &str, rom: &[u8]) {
    let hash = hash_rom_bytes(rom, spec);
    let dat = format!(
        "clrmamepro ( name \"{}\" )\n\
         game (\n\tname \"{game}\"\n\trom ( name \"{game}.{ext}\" size {} crc {} sha1 {} )\n)\n",
        spec.display_name, hash.size, hash.crc32, hash.sha1
    );
    std::fs::write(dir.join(spec.dat_file), dat).unwrap();
    let genre = format!(
        "game (\n\tcomment \"{game}\"\n\tgenre \"Puzzle\"\n\trom ( crc {} )\n)\n",
        hash.crc32
    );
    std::fs::create_dir_all(dir.join("metadat/genre")).unwrap();
    std::fs::write(dir.join("metadat/genre").join(spec.metadat_file), genre).unwrap();
}

#[test]
fn handheld_dumps_match_their_dat_with_metadata() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-handheld-db-")
        .tempdir()
        .unwrap();
    let (gb, gbc, gba) = (gb_rom(31), gb_rom(37), gba_rom());
    write_database(db.path(), &GB, "Fake Blocks (World)", "gb", &gb);
    write_database(db.path(), &GBC, "Fake Quest (World)", "gbc", &gbc);
    write_database(db.path(), &GBA, "Fake Advance (World)", "gba", &gba);

    let lib = TempLibrary::new("handheld-identify");
    lib.write("Handheld/blocks.gb", &gb);
    lib.write("Handheld/quest.GBC", &gbc);
    lib.write("Favorites/advance.gba", &gba);
    let mut headerless = gb.clone();
    headerless[0x104..0x108].fill(0);
    lib.write("bad/no-logo.gb", &headerless);
    let mut no_branch = gba.clone();
    no_branch[3] = 0;
    lib.write("bad/no-branch.gba", &no_branch);

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
        (3, 3, 0)
    );
    let mut reasons: Vec<(String, &str)> = summary
        .failures
        .iter()
        .map(|f| {
            let name = f.path.file_name().unwrap().to_string_lossy().into_owned();
            (name, f.reason.as_str())
        })
        .collect();
    reasons.sort();
    assert_eq!(
        reasons,
        [
            ("no-branch.gba".to_string(), "missing GBA header"),
            ("no-logo.gb".to_string(), "missing Game Boy header"),
        ]
    );

    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let expected = [
        ("blocks.gb", "gb", "Fake Blocks (World)"),
        ("quest.GBC", "gbc", "Fake Quest (World)"),
        ("advance.gba", "gba", "Fake Advance (World)"),
    ];
    for (file, system, name) in expected {
        let game = games
            .iter()
            .find(|g| g.rom_path.ends_with(file))
            .unwrap_or_else(|| panic!("{file} listed"));
        assert_eq!(game.platform_id, system, "{file}");
        assert_eq!(game.dat_name.as_deref(), Some(name), "{file}");
        let metadata = get_game_metadata(&lib.db.lock().unwrap(), game.id).unwrap();
        assert_eq!(metadata.unwrap().genre.as_deref(), Some("Puzzle"), "{file}");
    }
}

#[test]
fn vendored_handheld_dats_parse_with_complete_metadata() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    for spec in [GB, GBC, GBA] {
        let catalog = catalogs.get(spec.id);
        assert!(catalog.dat.len() > 1_000, "{} DAT entries", spec.id);
        assert!(catalog.metadata.is_complete(), "{} metadat", spec.id);
    }
    assert!(catalogs.loaded_complete());
}
