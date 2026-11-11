//! `fingerprint_source`: options hash, BIOS list, content crc (F4 / F27).

use system_catalog::{CORE_DEFAULTS, PSX, SNES};

use super::*;

fn opts(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

#[test]
fn options_hash_ignores_order_and_sees_multitap_options() {
    let a = opts(&[("x", "1"), ("y", "2")]);
    let b = opts(&[("y", "2"), ("x", "1")]);
    assert_eq!(options_hash(&a), options_hash(&b));
    assert_ne!(
        options_hash(&a),
        options_hash(&opts(&[("x", "1"), ("y", "3")]))
    );
    let two = crate::runner_core_options::resolve_for_players(&PSX, 2, &[]);
    let four = crate::runner_core_options::resolve_for_players(&PSX, 4, &[]);
    assert_ne!(
        options_hash(&two),
        options_hash(&four),
        "multitap option counted"
    );
}

#[test]
fn bios_list_is_sorted_or_hle() {
    let core = system_catalog::core_for_system(PSX.id).expect("PSX core");
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(bios_md5s(core, dir.path(), &[]), ["hle"], "no file: HLE");
    let forced = opts(&[("pcsx_rearmed_bios", "HLE")]);
    assert_eq!(bios_md5s(core, dir.path(), &forced), ["hle"]);
    for (i, file) in core.firmware.iter().take(2).enumerate() {
        let folder = dir.path().join(file.subdir);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(file.name), [i as u8; 16]).unwrap();
    }
    let md5s = bios_md5s(core, dir.path(), &[]);
    assert!(
        !md5s.is_empty() && md5s.windows(2).all(|w| w[0] <= w[1]),
        "{md5s:?}"
    );
    let none = CoreSpec {
        firmware: &[],
        ..CORE_DEFAULTS
    };
    assert!(bios_md5s(&none, dir.path(), &[]).is_empty());
}

#[test]
fn content_crc_uses_the_patched_bytes_and_skips_discs() {
    let dir = tempfile::tempdir().unwrap();
    let rom = dir.path().join("game.sfc");
    std::fs::write(&rom, b"original rom bytes").unwrap();
    let plain = SoftPatchedRom::default();
    let file_crc = content_crc32(Media::Cartridge, &plain, &rom);
    assert_eq!(file_crc, Some(crc32fast::hash(b"original rom bytes")));
    let patched = SoftPatchedRom {
        data: Some(b"patched rom bytes".to_vec()),
        title_label: None,
        patch_crc32: Some(7),
    };
    let patched_crc = content_crc32(Media::Cartridge, &patched, &rom);
    assert_ne!(patched_crc, file_crc);
    assert_eq!(content_crc32(Media::Disc, &plain, &rom), None);
    let core = system_catalog::core_for_system(SNES.id).expect("SNES core");
    let input = FingerprintInputs {
        core,
        system: &SNES,
        core_path: &rom, // any readable file stands in for the DLL
        core_version: "1.63",
        content_sha1: "abc",
        content_rom: &rom,
        patched: &patched,
        system_dir: dir.path(),
        core_options: &[],
        fps: 60.0988,
        portable_states: true,
        warmup_frames: 0,
        max_players: 2,
    };
    let fp = build_fingerprint(&input).unwrap();
    assert_eq!(
        (fp.patch_crc32, fp.fps_milli, fp.analog),
        (Some(7), 60_099, false)
    );
    assert_eq!(fp.core_sha256.len(), 64);
}
