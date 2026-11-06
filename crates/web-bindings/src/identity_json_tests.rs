use std::path::PathBuf;
use std::rc::Rc;

use content_file_source::{with_source, MemoryFiles};

use super::*;

fn test_rom(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-assets/roms")
        .join(name)
}

#[test]
fn classifies_and_identifies_like_the_desktop() {
    let empty = DatIndex::empty();
    for (name, want) in [
        ("nostyra-test.sfc", "snes"),
        ("nostyra-test.gb", "gb"),
        ("nostyra-test.gen", "genesis"),
        ("nostyra-test-psx.cue", "psx"),
    ] {
        let path = test_rom(name);
        assert_eq!(classify(&path), Ok(want), "{name}");
        let game = identify(&path, want, &empty, &MetadataIndex::empty()).unwrap();
        let sha1 = rom_identity::game_content_sha1(&path, system(want).unwrap()).unwrap();
        assert_eq!(game.sha1, sha1, "{name}");
        assert_eq!(game.content_id.as_deref(), Some(&sha1[..8]));
        assert_eq!(game.dat_name, None, "test ROMs are not in the DATs");
        assert!(game.title.starts_with("nostyra-test"), "{}", game.title);
    }
}

#[test]
fn dat_text_matches_by_hash() {
    let path = test_rom("nostyra-test.sfc");
    let game = identify(&path, "snes", &DatIndex::empty(), &MetadataIndex::empty()).unwrap();
    let dat = format!(
        "game (\n name \"Nostyra Test (World)\"\n region \"World\"\n rom ( name \"t.sfc\" size {} crc {} sha1 {} )\n)\n",
        game.size, game.crc32, game.sha1
    );
    let index = load_dat("snes", &dat).unwrap();
    let matched = identify(&path, "snes", &index, &MetadataIndex::empty()).unwrap();
    assert_eq!(matched.dat_name.as_deref(), Some("Nostyra Test (World)"));
    assert_eq!(matched.title, "Nostyra Test");
    assert!(load_dat("nope", &dat).is_err());
}

#[test]
fn virtual_paths_and_unknown_files() {
    let mut files = MemoryFiles::new();
    files.insert(
        "/web/1/a.nes",
        std::fs::read(test_rom("nostyra-test.nes")).unwrap(),
    );
    files.insert("/web/1/notes.txt", b"hello".to_vec());
    let (nes, txt) = with_source(Rc::new(files), || {
        (
            classify(Path::new("/web/1/a.nes")),
            classify(Path::new("/web/1/notes.txt")),
        )
    });
    assert_eq!(nes, Ok("nes"));
    assert!(txt.is_err());
}

#[test]
fn ips_patch_applies() {
    let rom = vec![0u8; 16];
    let mut ips = b"PATCH".to_vec();
    ips.extend_from_slice(&[0, 0, 4, 0, 2, 0xAB, 0xCD]);
    ips.extend_from_slice(b"EOF");
    let patched = apply_patch(&rom, &ips).unwrap();
    assert_eq!(&patched[3..7], &[0, 0xAB, 0xCD, 0]);
    assert!(apply_patch(&rom, b"nope").is_err());
}

#[test]
fn zip_single_rom_maps_scan_verdicts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("junk.zip");
    std::fs::write(&path, b"not a zip").unwrap();
    let allowed = vec!["snes".to_string()];
    assert!(matches!(
        zip_single_rom(&path, &allowed),
        ScanVerdict::Problem { .. }
    ));
}

#[test]
fn scan_verdicts_skip_non_roms_and_report_problems() {
    let dir = tempfile::tempdir().unwrap();
    let readme = dir.path().join("README.md");
    std::fs::write(&readme, b"# not a Mega Drive ROM").unwrap();
    assert_eq!(classify_for_scan(&readme), ScanVerdict::Skip);
    let rom = test_rom("nostyra-test.sfc");
    assert_eq!(
        classify_for_scan(&rom),
        ScanVerdict::Ok {
            system: "snes".into()
        }
    );
    let broken = dir.path().join("broken.nes");
    std::fs::write(&broken, vec![0u8; 40 * 1024]).unwrap();
    assert!(matches!(
        classify_for_scan(&broken),
        ScanVerdict::Problem { .. }
    ));
}
