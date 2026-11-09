//! Classifier tests for the Atari systems: per-extension probes (`.a78` / `.lnx`
//! headers, quiet `.o` / `.bll`) and the DAT-gated `.bin` the classifier skips.

use super::*;
use crate::NoDiscProbe;

fn head(ext: &str, bytes: &[u8]) -> Result<&'static SystemSpec, ClassifyError> {
    classify_with_head(
        &ALL_SYSTEMS,
        ext,
        bytes.len() as u64,
        Some(&bytes[..bytes.len().min(PROBE_BYTES)]),
        Path::new("mem"),
        &NoDiscProbe,
    )
}

fn id(ext: &str, bytes: &[u8]) -> Result<&'static str, ClassifyError> {
    head(ext, bytes).map(|spec| spec.id)
}

fn a78(with_magic: bool) -> Vec<u8> {
    let mut rom = vec![0xEAu8; 128 + 16 * 1024];
    if with_magic {
        rom[1..10].copy_from_slice(b"ATARI7800");
    }
    rom
}

fn lnx(with_magic: bool) -> Vec<u8> {
    let mut rom = vec![0xFFu8; 64 + 128 * 1024];
    if with_magic {
        rom[..5].copy_from_slice(b"LYNX\0");
    }
    rom
}

fn bs93() -> Vec<u8> {
    let mut rom = vec![0x80, 0x08, 0x02, 0x00, 0x00, 0x40];
    rom.extend_from_slice(b"BS93");
    rom.resize(64, 0xEA);
    rom
}

#[test]
fn a26_and_lyx_are_extension_only() {
    assert_eq!(id("a26", &[0u8; 4096]), Ok("atari2600"));
    assert_eq!(id("A26", &[0xFFu8; 2048]), Ok("atari2600"));
    assert_eq!(id("lyx", &[0u8; 128 * 1024]), Ok("lynx"));
    assert_eq!(id("a26", &[0u8; 1024]), Err(ClassifyError::OutOfSize));
}

#[test]
fn a78_and_lnx_classify_with_or_without_their_header() {
    // GoodTools names headerless dumps `.a78` / `.lnx` (No-Intro `.bin` / `.lyx` bytes).
    for with_header in [true, false] {
        assert_eq!(id("a78", &a78(with_header)), Ok("atari7800"));
        assert_eq!(id("lnx", &lnx(with_header)), Ok("lynx"));
    }
}

#[test]
fn bs93_programs_are_lynx_and_object_files_are_ignored() {
    for ext in ["o", "bll", "O"] {
        assert_eq!(id(ext, &bs93()), Ok("lynx"), "{ext}");
    }
    let mut elf = b"\x7fELF\x02\x01\x01".to_vec();
    elf.resize(512, 0);
    let mut coff = vec![0x64, 0x86, 0x02, 0x00];
    coff.resize(300, 0);
    for object in [elf, coff] {
        assert!(matches!(id("o", &object), Err(ClassifyError::NotRom(_))));
    }
}

#[test]
fn bin_is_skipped_by_the_atari_systems() {
    // Mega Drive still wins with its magic.
    let mut md = vec![0u8; 0x20000];
    md[0x100..0x104].copy_from_slice(b"SEGA");
    assert_eq!(id("bin", &md), Ok("genesis"));
    // An unknown 4 KiB `.bin` gets exactly the PlayStation reject (no Atari text).
    let plain = head("bin", &[0x4Cu8; 4096]);
    let only_psx = classify_with_head(
        &[&crate::GENESIS, &crate::PSX],
        "bin",
        4096,
        Some(&[0x4Cu8; 4096][..PROBE_BYTES]),
        Path::new("mem"),
        &NoDiscProbe,
    );
    assert!(matches!(plain, Err(ClassifyError::DiscRejected(_))));
    assert_eq!(plain, only_psx);
    // Atari-only candidates: no result, no error beyond "no candidate".
    let atari_only = classify_with_head(
        &[&crate::ATARI2600, &crate::ATARI7800],
        "bin",
        4096,
        Some(&[0u8; PROBE_BYTES][..]),
        Path::new("mem"),
        &NoDiscProbe,
    );
    assert_eq!(atari_only, Err(ClassifyError::NoCandidate));
}

#[test]
fn dat_candidates_are_the_atari_bin_systems_by_size() {
    let ids = |ext: &str, len: u64| -> Vec<&str> {
        crate::dat_identified_candidates(&ALL_SYSTEMS, ext, len)
            .iter()
            .map(|spec| spec.id)
            .collect()
    };
    assert_eq!(ids("bin", 4096), ["atari2600", "atari7800"]);
    assert_eq!(ids("BIN", 2048), ["atari2600"], "7800 starts at 4 KiB");
    assert_eq!(ids("bin", 1024 * 1024 + 128), ["atari7800"]);
    assert!(ids("bin", 2 * 1024 * 1024).is_empty());
    assert!(
        ids("a26", 4096).is_empty(),
        "named extensions are classified"
    );
    assert!(ids("a78", 4096).is_empty());
}
