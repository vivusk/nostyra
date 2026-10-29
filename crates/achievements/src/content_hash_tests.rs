//! Hash parity: cartridges vs a Python md5 reference, disc fixtures with rcheevos'
//! default readers vs ours, the patched-buffer path, Unicode paths, request rules.

use std::path::{Path, PathBuf};

use system_catalog::{GBA, NES, PSP, PSX, WII};

use super::*;

pub(super) fn roms() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms")
}

pub(super) fn default_hash(console: u32, path: &Path) -> Option<String> {
    generate_hash(console, path.to_str()?, None, HashReaders::Default)
}

/// `python -I rc_md5_ref.py` (rcheevos rules re-implemented in Python), computed on
/// 2026-10-28 (M15 spike C; recomputed in phase 07 after the frame-counter RAM copies).
const CARTRIDGES: &[(&str, u32, &str)] = &[
    ("nostyra-test.nes", 7, "56479a1eae301c86c803c0caeab8935f"),
    ("nostyra-test.sfc", 3, "7a0c0dc01bc47346e71c8a797fc09ee4"),
    ("nostyra-test.gb", 4, "9a4cf57700a59e384d4e58a7c54e82d6"),
    ("nostyra-test.gbc", 6, "06858f875e98f9ebf692a4df939f81d1"),
    ("nostyra-test.gba", 5, "7bae559c191e9664b0c1008d2add6533"),
    ("nostyra-test.gen", 1, "848d85d8f4ff37f0831b9dfe26d33b05"),
    ("nostyra-test.sms", 11, "756102f9eff77cf6ba653a8077e20349"),
    ("nostyra-test.gg", 15, "599d451aadb27125b3a691dbe4462548"),
    ("nostyra-test.sg", 33, "a3f6fcf859a53c037c41fccf5742cba4"),
    ("nostyra-test.z64", 2, "29e45e317ae6d40f7e665048da86508a"),
    ("nostyra-test.nds", 18, "bb170d92aa660083da1f4a3ce714e0bb"),
    (
        "nostyra-test-save.nds",
        18,
        "59529a3ff831b2aaec8ba9bea0d42324",
    ),
    (
        "nostyra-test-psp/EBOOT.PBP",
        41,
        "33671d0d1a274af9077295f11768aa93",
    ),
];

pub(super) const PSX_HASH: &str = "1a6bda47ed75db642b078ca628b17c14";
pub(super) const DC_HASH: &str = "e409cbc65e2c90b7e37218815787e0b4";
pub(super) const PSP_HASH: &str = "12401395fbb18b25dfd26ead32cb6999";

#[test]
fn cartridges_match_the_python_reference_with_both_readers() {
    for (name, console, reference) in CARTRIDGES {
        let path = roms().join(name);
        let (default, ours) = (
            default_hash(*console, &path),
            content_hash(*console, &path, None),
        );
        assert_eq!(default.as_deref(), Some(*reference), "{name} default");
        assert_eq!(ours.as_deref(), Some(*reference), "{name} ours");
    }
}

#[test]
fn disc_fixtures_match_the_default_reader() {
    for (name, console, expected) in [
        ("nostyra-test-psx.cue", 12, PSX_HASH),
        ("nostyra-test-psx.bin", 12, PSX_HASH),
        ("nostyra-test-psx.m3u", 12, PSX_HASH),
        (
            "nostyra-test-psx-disc2.cue",
            12,
            "014e57d0f17ebacac079d995e09ca260",
        ),
        ("nostyra-test-dc.gdi", 40, DC_HASH),
        ("nostyra-test-dc.m3u", 40, DC_HASH),
        (
            "nostyra-test-dc-disc2.gdi",
            40,
            "a82d8e9f7ab243f9b406882350e68617",
        ),
        ("nostyra-test-psp.iso", 41, PSP_HASH),
    ] {
        let path = roms().join(name);
        let (default, ours) = (
            default_hash(console, &path),
            content_hash(console, &path, None),
        );
        assert_eq!(default.as_deref(), Some(expected), "{name} default");
        assert_eq!(ours.as_deref(), Some(expected), "{name} ours");
    }
}

#[test]
fn redump_gd_cue_and_cso_need_our_reader() {
    let cue = roms().join("nostyra-test-dc.cue");
    assert_eq!(
        default_hash(40, &cue),
        None,
        "default lacks the GD 45000 offset"
    );
    assert_eq!(content_hash(40, &cue, None).as_deref(), Some(DC_HASH));
    let cso = roms().join("nostyra-test-psp.cso");
    assert_eq!(default_hash(41, &cso), None, "default has no CSO reader");
    assert_eq!(content_hash(41, &cso, None).as_deref(), Some(PSP_HASH));
}

#[test]
fn patched_cartridge_bytes_change_the_hash() {
    for (name, console, reference) in CARTRIDGES.iter().filter(|c| c.1 != 41) {
        let path = roms().join(name);
        let mut bytes = std::fs::read(&path).unwrap();
        let same = content_hash(*console, &path, Some(&bytes));
        assert_eq!(same.as_deref(), Some(*reference), "{name}");
        bytes[0x100] ^= 0xFF; // inside every hashed region (NDS: the header)
        let patched = content_hash(*console, &path, Some(&bytes));
        assert!(patched.is_some() && patched != same, "{name}");
    }
}

#[test]
fn unicode_paths_hash() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("ゲーム ü");
    std::fs::create_dir(&sub).unwrap();
    let path = sub.join("ゲーム.gba");
    std::fs::copy(roms().join("nostyra-test.gba"), &path).unwrap();
    let hash = content_hash(5, &path, None);
    assert_eq!(hash.as_deref(), Some("7bae559c191e9664b0c1008d2add6533"));
}

#[test]
fn hash_request_passes_patched_bytes_for_cartridges_only() {
    let rom = Path::new("game.gba");
    let request = hash_request(&GBA, rom, Some(vec![1, 2])).unwrap();
    assert_eq!((request.console_id, request.data), (5, Some(vec![1, 2])));
    assert_eq!(hash_request(&NES, rom, None).unwrap().data, None);
    let disc = hash_request(&PSX, Path::new("game.cue"), Some(vec![1])).unwrap();
    assert_eq!((disc.console_id, disc.data), (12, None));
}

#[test]
fn ps1_eboot_and_wii_are_unavailable_but_psp_eboot_is_not() {
    let eboot = Path::new("EBOOT.PBP");
    assert_eq!(
        hash_request(&PSX, eboot, None),
        Err(HashUnavailable::Format)
    );
    assert_eq!(hash_request(&PSP, eboot, None).unwrap().console_id, 41);
    let wii = hash_request(&WII, Path::new("game.iso"), None);
    assert_eq!(wii, Err(HashUnavailable::System));
}

#[test]
fn the_client_hashes_through_our_readers() {
    use crate::achievement_client::test_support::{session, wait_for};
    use crate::{ClientOp, RequestDispatch, RC_NO_GAME_LOADED};
    let s = session(RequestDispatch::Inline, false);
    s.client.begin_login_with_token("Tester", "tok123");
    let cue = roms().join("nostyra-test-dc.cue");
    s.client.begin_identify_and_load_game(40, &cue, None);
    assert_eq!(
        wait_for(&s.client, ClientOp::LoadGame).result,
        RC_NO_GAME_LOADED
    );
    let game = s.client.game_info().unwrap();
    assert_eq!(game.hash, DC_HASH, "Redump GD cue needs our cdreader");
}

#[path = "content_hash_container_tests.rs"]
mod container_tests;
