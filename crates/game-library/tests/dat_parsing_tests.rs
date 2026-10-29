//! clrmamepro parser, DatIndex lookups, title/region helpers, bundled DAT.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use game_library::{
    display_region, display_title, hash_rom_bytes, parse_clrmamepro_dat, DatIndex, LibraryError,
    RomHashes,
};

const SAMPLE: &str = r#"
clrmamepro (
	name "Nintendo - Super Nintendo Entertainment System"
	version "2026.10.07"
)

game (
	name "Super Mario World (USA)"
	region "USA"
	serial "SNS-MW-USA"
	rom ( name "Super Mario World (USA).sfc" size 524288 crc b19ed489 md5 CDD3C8C37322978CA8669B34BC89C804 sha1 6b47bb75d16514b6a476aa0c73a683a2a4c18765 )
)
game (
	name "Pokémon Kid (Europe) (Unl)"
	rom ( name "a.sfc" size 1024 crc 00000001 )
	rom ( name "b.sfc" size 2048 crc 00000002 sha1 BBBB )
)
"#;

#[test]
fn parses_games_roms_and_skips_unknown_keys() {
    let entries = parse_clrmamepro_dat(SAMPLE).unwrap();
    assert_eq!(entries.len(), 3);
    let smw = &entries[0];
    assert_eq!(smw.name, "Super Mario World (USA)");
    assert_eq!(smw.region.as_deref(), Some("USA"));
    assert_eq!((smw.size, smw.crc32.as_str()), (524_288, "B19ED489"));
    assert_eq!(
        smw.sha1.as_deref(),
        Some("6B47BB75D16514B6A476AA0C73A683A2A4C18765")
    );
    // Multi-ROM game: one entry per ROM, missing region and sha1 tolerated.
    assert_eq!(entries[1].name, "Pokémon Kid (Europe) (Unl)");
    assert_eq!(
        (entries[1].region.clone(), entries[1].sha1.clone()),
        (None, None)
    );
    assert_eq!(entries[2].sha1.as_deref(), Some("BBBB"));
}

#[test]
fn backslashes_are_literal_because_libretro_dats_have_no_escapes() {
    let dat = r#"game ( name "Back\slash" rom ( name "a.sfc" size 1 crc 0000000A ) )"#;
    let entries = parse_clrmamepro_dat(dat).unwrap();
    assert_eq!(entries[0].name, r"Back\slash");
}

#[test]
fn malformed_dats_are_errors() {
    assert!(parse_clrmamepro_dat("game ( name \"x\" ").is_err());
    assert!(parse_clrmamepro_dat("game ( name \"unterminated )").is_err());
    assert!(parse_clrmamepro_dat("game ( rom ( size 1 crc 0 ) )").is_err());
    assert!(parse_clrmamepro_dat("stray").is_err());
}

#[test]
fn lookup_prefers_sha1_then_crc_and_size() {
    let index = DatIndex::from_entries(parse_clrmamepro_dat(SAMPLE).unwrap());
    let by_sha1 = RomHashes {
        crc32: "FFFFFFFF".into(),
        sha1: "6B47BB75D16514B6A476AA0C73A683A2A4C18765".into(),
        size: 1,
    };
    assert_eq!(
        index.lookup(&by_sha1).unwrap().name,
        "Super Mario World (USA)"
    );
    let by_crc = RomHashes {
        crc32: "00000001".into(),
        sha1: "NOPE".into(),
        size: 1024,
    };
    assert_eq!(
        index.lookup(&by_crc).unwrap().name,
        "Pokémon Kid (Europe) (Unl)"
    );
    let wrong_size = RomHashes {
        size: 4096,
        ..by_crc
    };
    assert!(index.lookup(&wrong_size).is_none());
    assert!(DatIndex::empty().lookup(&by_sha1).is_none());
}

#[test]
fn titles_and_regions() {
    assert_eq!(
        display_title("Super Mario World (USA)"),
        "Super Mario World"
    );
    assert_eq!(display_title("Chrono Trigger (U) [!]"), "Chrono Trigger");
    assert_eq!(display_title("Plain Name"), "Plain Name");
    assert_eq!(display_title("(Prototype)"), "(Prototype)");
    let entries = parse_clrmamepro_dat(SAMPLE).unwrap();
    assert_eq!(display_region(&entries[0]).as_deref(), Some("USA"));
    assert_eq!(display_region(&entries[1]).as_deref(), Some("Europe"));
}

#[test]
fn missing_dat_file_is_an_error_and_empty_index_is_the_fallback() {
    let err = DatIndex::load(Path::new("does/not/exist.dat")).unwrap_err();
    assert!(matches!(err, LibraryError::Io { .. }));
    let index =
        DatIndex::load(Path::new("does/not/exist.dat")).unwrap_or_else(|_| DatIndex::empty());
    assert!(index.is_empty());
}

#[test]
fn bundled_dat_loads_and_identifies_known_dump() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../third-party/libretro-database/Nintendo - Super Nintendo Entertainment System.dat",
    );
    let index = DatIndex::load(&path).unwrap();
    assert!(index.len() >= 3_700, "only {} entries", index.len());
    let known = RomHashes {
        crc32: "05FBB855".into(),
        sha1: "005CCD8362DC41491F89F31FC9326A6688300E0C".into(),
        size: 1_572_864,
    };
    let entry = index.lookup(&known).unwrap();
    assert_eq!(entry.name, "'96 Zenkoku Koukou Soccer Senshuken (Japan)");
    assert_eq!(display_region(entry).as_deref(), Some("Japan"));

    // The self-written test ROM is not a No-Intro dump.
    let test_rom = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms/nostyra-test.sfc"),
    )
    .unwrap();
    assert!(index
        .lookup(&hash_rom_bytes(&test_rom, &system_catalog::SNES))
        .is_none());
}
