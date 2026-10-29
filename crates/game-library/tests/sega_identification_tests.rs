//! Mega Drive, Master System, Game Gear and SG-1000 identification: raw `.md`, its
//! SMD copy and a `.bin` match the same No-Intro entry (SMD hashes as the raw
//! image, so the content id and save folder are shared), 8-bit files hash whole, a
//! Markdown `README.md` is neither listed nor reported.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{game_content_sha1, hash_rom_bytes, scan_library, SystemCatalogs};
use storage_db::{get_game_metadata, list_games, GameListQuery};
use system_catalog::{raw_to_smd, SystemSpec, GENESIS, GG, SG1000, SMS};

/// 128 KiB raw Mega Drive image (No-Intro layout: "SEGA" at 0x100).
fn mega_drive_rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..0x20000u32).map(|i| (i * 29 % 251) as u8).collect();
    rom[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
    rom
}

fn eight_bit_rom(seed: u32) -> Vec<u8> {
    (0..0x8000u32)
        .map(|i| (i.wrapping_mul(seed) % 253) as u8)
        .collect()
}

/// Writes `<dir>/<dat>` + `<dir>/metadat/genre/<dat>` naming `rom` as `game`.
fn write_database(dir: &Path, spec: &SystemSpec, game: &str, rom: &[u8]) {
    let hash = hash_rom_bytes(rom, spec);
    let dat = format!(
        "clrmamepro ( name \"{}\" )\n\
         game (\n\tname \"{game}\"\n\trom ( name \"{game}.{}\" size {} crc {} sha1 {} )\n)\n",
        spec.display_name, spec.extensions[0], hash.size, hash.crc32, hash.sha1
    );
    std::fs::write(dir.join(spec.dat_file), dat).unwrap();
    let genre = format!(
        "game (\n\tcomment \"{game}\"\n\tgenre \"Action\"\n\trom ( crc {} )\n)\n",
        hash.crc32
    );
    std::fs::create_dir_all(dir.join("metadat/genre")).unwrap();
    std::fs::write(dir.join("metadat/genre").join(spec.metadat_file), genre).unwrap();
}

#[test]
fn sega_dumps_match_their_dat_with_metadata() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-sega-db-")
        .tempdir()
        .unwrap();
    let md = mega_drive_rom();
    let (sms, gg, sg) = (eight_bit_rom(31), eight_bit_rom(37), eight_bit_rom(41));
    write_database(db.path(), &GENESIS, "Fake Hedgehog (World)", &md);
    write_database(db.path(), &SMS, "Fake Ninja (Europe)", &sms);
    write_database(db.path(), &GG, "Fake Pocket (World)", &gg);
    write_database(db.path(), &SG1000, "Fake Classic (Japan)", &sg);

    let lib = TempLibrary::new("sega-identify");
    lib.write("Sega/hedgehog.md", &md);
    lib.write("Sega/hedgehog-copier.smd", &raw_to_smd(&md));
    lib.write("Mixed/hedgehog.bin", &md);
    lib.write("Sega/ninja.sms", &sms);
    lib.write("Sega/pocket.GG", &gg);
    lib.write("Sega/classic.sg", &sg);
    let readme = "# Sega dumps\n\nVerified against No-Intro.\n".repeat(40);
    lib.write("Sega/README.md", readme.as_bytes());

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
        (6, 6, 0)
    );
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);

    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let expected = [
        ("hedgehog.md", "genesis", "Fake Hedgehog (World)"),
        ("hedgehog-copier.smd", "genesis", "Fake Hedgehog (World)"),
        ("hedgehog.bin", "genesis", "Fake Hedgehog (World)"),
        ("ninja.sms", "sms", "Fake Ninja (Europe)"),
        ("pocket.GG", "gg", "Fake Pocket (World)"),
        ("classic.sg", "sg1000", "Fake Classic (Japan)"),
    ];
    for (file, system, name) in expected {
        let game = games
            .iter()
            .find(|g| g.rom_path.ends_with(file))
            .unwrap_or_else(|| panic!("{file} listed"));
        assert_eq!(game.platform_id, system, "{file}");
        assert_eq!(game.dat_name.as_deref(), Some(name), "{file}");
        let metadata = get_game_metadata(&lib.db.lock().unwrap(), game.id).unwrap();
        assert_eq!(metadata.unwrap().genre.as_deref(), Some("Action"), "{file}");
    }
    assert!(games.iter().all(|g| !g.rom_path.ends_with("README.md")));
}

#[test]
fn smd_copy_shares_the_raw_content_id() {
    let lib = TempLibrary::new("sega-content-id");
    let md = mega_drive_rom();
    let raw = lib.write("raw.md", &md);
    let smd = lib.write("copier.smd", &raw_to_smd(&md));
    let raw_id = game_content_sha1(&raw, &GENESIS).unwrap();
    let smd_id = game_content_sha1(&smd, &GENESIS).unwrap();
    assert_eq!(raw_id, smd_id);
    assert_eq!(raw_id, hash_rom_bytes(&md, &GENESIS).sha1);
}

#[test]
fn vendored_sega_dats_parse_with_complete_metadata() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    for spec in [GENESIS, SMS, GG, SG1000] {
        let catalog = catalogs.get(spec.id);
        assert!(catalog.dat.len() > 100, "{} DAT entries", spec.id);
        assert!(catalog.metadata.is_complete(), "{} metadat", spec.id);
    }
    assert!(catalogs.loaded_complete());
}
