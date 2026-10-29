//! Multi-file disc games in a folder walk: one entry per game, referenced files
//! hidden (with their probe failures), broken sheets reported once.
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::path::Path;

use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
use game_library::{discover_rom_files, DiscoveredFiles};
use system_catalog::ALL_SYSTEMS;

fn ps1_disc(serial_exe: &str) -> Vec<u8> {
    to_raw(
        &ps1_sectors(Some(US_LICENCE), &format!(r"BOOT = cdrom:\{serial_exe};1")),
        2,
    )
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn cue_text(files: &[(&str, &str)]) -> String {
    files
        .iter()
        .enumerate()
        .map(|(i, (name, mode))| {
            format!(
                "FILE \"{name}\" BINARY\n  TRACK {:02} {mode}\n    INDEX 01 00:00:00\n",
                i + 1
            )
        })
        .collect()
}

fn names(found: &DiscoveredFiles) -> Vec<String> {
    let mut names: Vec<String> = found
        .files
        .iter()
        .map(|f| {
            Path::new(&f.path)
                .strip_prefix(found_root(f.path.as_str()))
                .unwrap_or(Path::new(&f.path))
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    names.sort();
    names
}

/// Test roots are temp dirs named `nostyra-group-*`; names are shown relative to them.
fn found_root(path: &str) -> &Path {
    let mut root = Path::new(path);
    while let Some(parent) = root.parent() {
        if root
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("nostyra-group-"))
        {
            break;
        }
        root = parent;
    }
    root
}

fn temp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nostyra-group-")
        .tempdir()
        .unwrap()
}

#[test]
fn cue_with_tracks_is_one_game_and_track_failures_vanish() {
    let dir = temp();
    let disc = ps1_disc("SLUS_012.34");
    put(dir.path(), "G (Track 1).bin", &disc);
    put(dir.path(), "G (Track 2).bin", &[5u8; 2352 * 10]);
    put(dir.path(), "G (Track 3).bin", &[6u8; 2352 * 10]);
    let cue = cue_text(&[
        ("G (Track 1).bin", "MODE2/2352"),
        ("G (Track 2).bin", "AUDIO"),
        ("G (Track 3).bin", "AUDIO"),
    ]);
    put(dir.path(), "G.cue", cue.as_bytes());
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    assert_eq!(names(&found), ["G.cue"]);
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    let total = cue.len() + disc.len() + 2 * 2352 * 10;
    assert_eq!(
        found.files[0].size, total as u64,
        "fingerprint covers all files"
    );
    assert_eq!(found.files[0].system.id, "psx");
}

#[test]
fn playlist_hides_its_discs_also_in_subfolders() {
    let dir = temp();
    for (n, exe) in [(1, "SLUS_000.01"), (2, "SLUS_000.02")] {
        put(dir.path(), &format!("discs/D{n}.bin"), &ps1_disc(exe));
        let cue = cue_text(&[(&format!("D{n}.bin"), "MODE2/2352")]);
        put(dir.path(), &format!("discs/D{n}.cue"), cue.as_bytes());
    }
    put(dir.path(), "Game.m3u", b"discs/D1.cue\ndiscs/D2.cue\n");
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    assert_eq!(names(&found), ["Game.m3u"]);
    assert!(found.failures.is_empty(), "{:?}", found.failures);
}

#[test]
fn same_stem_bin_beside_a_cue_is_hidden() {
    let dir = temp();
    put(dir.path(), "Game (Track 1).bin", &ps1_disc("SLUS_012.34"));
    put(dir.path(), "Game.bin", &ps1_disc("SLUS_012.34"));
    let cue = cue_text(&[("Game (Track 1).bin", "MODE2/2352")]);
    put(dir.path(), "Game.cue", cue.as_bytes());
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    assert_eq!(names(&found), ["Game.cue"]);
}

#[test]
fn broken_cue_in_a_flat_folder_hides_only_its_own_files() {
    let dir = temp();
    // Brigandine-like: FILE line 1 names a missing bin; the real bin was renamed.
    put(dir.path(), "Brig_VIET_TGGR.bin", &ps1_disc("SLUS_004.59"));
    put(dir.path(), "Brig (Track 02).bin", &[1u8; 2352 * 4]);
    let cue = cue_text(&[
        ("Brig_VIET_D13.bin", "MODE2/2352"),
        ("Brig (Track 02).bin", "AUDIO"),
    ]);
    put(dir.path(), "Brig_VIET_TGGR.cue", cue.as_bytes());
    // Unrelated bare-bin games in the same folder stay listed.
    put(dir.path(), "CTR (US).bin", &ps1_disc("SCUS_944.26"));
    put(dir.path(), "Emblem Saga (VN).bin", &ps1_disc("SLPS_031.77"));
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    assert_eq!(names(&found), ["CTR (US).bin", "Emblem Saga (VN).bin"]);
    assert_eq!(found.failures.len(), 1, "{:?}", found.failures);
    assert!(found.failures[0].path.ends_with("Brig_VIET_TGGR.cue"));
    assert_eq!(
        found.failures[0].reason,
        "track file 'Brig_VIET_D13.bin' not found — fix the FILE line or rename the file"
    );
}

#[test]
fn bare_bin_is_a_game() {
    let dir = temp();
    put(dir.path(), "Solo.bin", &ps1_disc("SLUS_012.34"));
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    assert_eq!(names(&found), ["Solo.bin"]);
    assert!(found.failures.is_empty());
}

#[test]
fn repo_test_discs_group_under_their_playlist() {
    let roms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    let found = discover_rom_files(&roms, &ALL_SYSTEMS);
    let psx: Vec<String> = found
        .files
        .iter()
        .filter(|f| f.system.id == "psx")
        .map(|f| {
            Path::new(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        psx,
        ["nostyra-test-psx.m3u"],
        "both discs and tracks hidden"
    );
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    let serial = |file: &str| {
        let mut reader = disc_image::open_data_track(&roms.join(file)).unwrap();
        disc_image::serial(reader.as_mut()).unwrap()
    };
    assert_eq!(
        serial("nostyra-test-psx.cue").as_deref(),
        Some("NSTY-00001")
    );
    assert_eq!(
        serial("nostyra-test-psx-disc2.cue").as_deref(),
        Some("NSTY-00002")
    );
}
