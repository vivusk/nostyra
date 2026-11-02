//! Archive scans use the ROM's identity and the ZIP's filesystem fingerprint.
#![allow(clippy::unwrap_used)]

mod common;

use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::{fake_rom, snes_catalogs, TempLibrary};
use game_library::{
    discover_rom_files, discover_rom_files_with_options, game_content_sha1, hash_rom_bytes,
    read_rom_archive, scan_library_with_options, DatEntry, DatIndex, MetadataIndex, ScanOptions,
    ScanSummary, SystemCatalogs,
};
use storage_db::{list_games, GameListQuery};
use system_catalog::ALL_SYSTEMS;
use zip::{write::SimpleFileOptions, ZipWriter};

fn archive(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn scan(lib: &TempLibrary, catalogs: &SystemCatalogs, enabled: bool) -> ScanSummary {
    scan_library_with_options(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
        ScanOptions {
            scan_zip_archives: enabled,
        },
    )
    .unwrap()
}

fn games(lib: &TempLibrary) -> Vec<storage_db::Game> {
    list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap()
}

#[test]
fn scans_only_allowed_systems_with_original_rom_hashes() {
    let lib = TempLibrary::new("zip-systems");
    for (ext, id) in [
        ("nes", "nes"),
        ("sfc", "snes"),
        ("gen", "genesis"),
        ("sms", "sms"),
        ("gg", "gg"),
        ("sg", "sg1000"),
        ("gba", "gba"),
    ] {
        let mut bytes = fake_rom(32 * 1024, 11);
        bytes[..4].copy_from_slice(b"NES\x1a");
        bytes[3] = if ext == "gba" { 0xea } else { 0x1a };
        let member = format!("folder/Member.{ext}");
        let path = lib.roms().join(format!("{id}.ZIP"));
        archive(
            &path,
            &[
                (member.as_str(), &bytes),
                ("README.md", b"a markdown document"),
            ],
        );
        let content = read_rom_archive(&path).unwrap();
        assert_eq!(content.system.id, id);
        assert_eq!(content.bytes, bytes);
        assert_eq!(
            game_content_sha1(&path, content.system).unwrap(),
            hash_rom_bytes(&bytes, content.system).sha1
        );
    }
    assert!(discover_rom_files(&lib.roms(), &ALL_SYSTEMS)
        .files
        .is_empty());
    let result = scan(&lib, &SystemCatalogs::new(None), true);
    assert_eq!(result.added, 7, "{result:?}");
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    for game in games(&lib) {
        assert_eq!(game.title, "Member");
        assert!(game.rom_path.ends_with(".ZIP"));
        assert_eq!(
            game.file_size as u64,
            std::fs::metadata(&game.rom_path).unwrap().len()
        );
    }
}

#[test]
fn headered_snes_matches_dat_and_unchanged_zip_is_rematched_by_rom_size() {
    let lib = TempLibrary::new("zip-dat");
    let bytes = fake_rom(32 * 1024, 9);
    let mut headered = vec![0; 512];
    headered.extend_from_slice(&bytes);
    let path = lib.roms().join("Different archive name.zip");
    archive(&path, &[("Inner Game (Japan).smc", &headered)]);
    let empty = SystemCatalogs::new(None);
    assert_eq!(scan(&lib, &empty, true).added, 1);
    assert_eq!(games(&lib)[0].title, "Inner Game");
    assert_eq!(scan(&lib, &empty, true).unchanged, 1);
    let hash = hash_rom_bytes(&bytes, &system_catalog::SNES);
    let dat = DatIndex::from_entries(vec![DatEntry {
        name: "Identified Game (USA)".into(),
        region: None,
        size: hash.size,
        crc32: hash.crc32.clone(),
        metadata_crc32: hash.crc32.clone(),
        sha1: None,
        serial: None,
    }]);
    let catalogs = snes_catalogs(&dat, &MetadataIndex::empty());
    assert_eq!(scan(&lib, &catalogs, true).rematched, 1);
    let game = &games(&lib)[0];
    assert_eq!(game.title, "Identified Game");
    assert_eq!((&game.crc32, &game.sha1), (&hash.crc32, &hash.sha1));
    assert_eq!(game.rom_path, path.to_str().unwrap());
    // A fresh scan must also match by the canonical uncompressed size.
    let fresh = TempLibrary::new("zip-dat-fresh");
    archive(&fresh.roms().join("copy.zip"), &[("copy.smc", &headered)]);
    assert_eq!(scan(&fresh, &catalogs, true).matched, 1);
}

#[test]
fn disabling_scan_preserves_present_games_but_deleted_archives_become_missing() {
    let lib = TempLibrary::new("zip-off");
    let path = lib.roms().join("game.zip");
    archive(&path, &[("game.sfc", &fake_rom(32 * 1024, 5))]);
    let catalogs = SystemCatalogs::new(None);
    assert_eq!(scan(&lib, &catalogs, false).added, 0);
    assert_eq!(scan(&lib, &catalogs, true).added, 1);
    assert_eq!(scan(&lib, &catalogs, false).missing, 0);
    assert!(!games(&lib)[0].missing);
    archive(&path, &[("game.sfc", &fake_rom(64 * 1024, 6))]);
    let before = games(&lib)[0].sha1.clone();
    assert_eq!(scan(&lib, &catalogs, false).updated, 0);
    assert_eq!(games(&lib)[0].sha1, before);
    assert_eq!(scan(&lib, &catalogs, true).updated, 1);
    assert_ne!(games(&lib)[0].sha1, before);
    std::fs::remove_file(path).unwrap();
    assert_eq!(scan(&lib, &catalogs, false).missing, 1);
    assert!(games(&lib)[0].missing);
}

#[test]
fn rejects_multiple_unsupported_unsafe_and_corrupt_archives_without_stopping_scan() {
    let lib = TempLibrary::new("zip-invalid");
    let rom = fake_rom(32 * 1024, 8);
    for (name, entries) in [
        (
            "two.zip",
            vec![("one.sfc", rom.as_slice()), ("two.sfc", rom.as_slice())],
        ),
        ("unsupported.zip", vec![("one.gb", rom.as_slice())]),
        (
            "mixed.zip",
            vec![("one.sfc", rom.as_slice()), ("other.gba", rom.as_slice())],
        ),
        ("unsafe.zip", vec![("../escape.sfc", rom.as_slice())]),
        ("windows.zip", vec![("C:\\escape.sfc", rom.as_slice())]),
        ("empty.zip", vec![("readme.txt", b"no ROM".as_slice())]),
        ("nested.zip", vec![("inner.zip", b"archive".as_slice())]),
        ("bad-nes.zip", vec![("bad.nes", rom.as_slice())]),
    ] {
        let path = lib.roms().join(name);
        archive(&path, &entries);
        assert!(read_rom_archive(&path).is_err(), "{name}");
    }
    lib.write("broken.zip", b"not a zip");
    lib.write("good.sfc", &rom);
    let result = scan(&lib, &SystemCatalogs::new(None), true);
    assert_eq!(result.added, 1);
    assert_eq!(result.failures.len(), 9, "{result:?}");
}

#[test]
fn ignores_macos_metadata_and_obeys_system_filter_and_size_limit() {
    let lib = TempLibrary::new("zip-metadata");
    let rom = fake_rom(32 * 1024, 4);
    let path = lib.roms().join("game.zip");
    archive(
        &path,
        &[
            ("__MACOSX/._game.sfc", b"metadata"),
            ("folder/._game.sfc", b"metadata"),
            ("folder/Game.SFC", &rom),
        ],
    );
    assert_eq!(read_rom_archive(&path).unwrap().bytes, rom);
    let filtered = discover_rom_files_with_options(
        &lib.roms(),
        &[&system_catalog::NES],
        ScanOptions {
            scan_zip_archives: true,
        },
    );
    assert!(filtered.files.is_empty());
    let oversized = vec![0; system_catalog::SNES.max_bytes as usize + 1];
    archive(&path, &[("too-big.sfc", &oversized)]);
    assert!(read_rom_archive(&path).is_err());
}
