//! Classifier tests for the Sega systems: `.bin` shared with disc images and `.md`
//! shared with Markdown documents are decided by the `SEGA` magic.

use super::*;
use crate::{raw_to_smd, NoDiscProbe};

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

/// 128 KiB raw Mega Drive image with `SEGA` at `magic_at` (None: no magic).
fn mega_drive(magic_at: Option<usize>) -> Vec<u8> {
    let mut rom = vec![0x4Eu8; 0x20000];
    rom[0x100..0x110].fill(b' ');
    if let Some(at) = magic_at {
        rom[at..at + 4].copy_from_slice(b"SEGA");
    }
    rom
}

#[test]
fn bin_and_md_need_the_sega_magic() {
    for ext in ["bin", "BIN", "md"] {
        for at in [0x100, 0x101] {
            assert_eq!(
                head(ext, &mega_drive(Some(at))).map(|s| s.id),
                Ok("genesis"),
                "{ext} @ {at:#x}"
            );
        }
    }
    // `.bin` without the magic falls through to the PlayStation disc probe.
    assert!(matches!(
        head("bin", &mega_drive(None)),
        Err(ClassifyError::DiscRejected(_))
    ));
    // A short `.bin` (magic cut off) is no Mega Drive either.
    assert!(matches!(
        head("bin", &mega_drive(Some(0x100))[..0x102]),
        Err(ClassifyError::DiscRejected(_))
    ));
    // SMD layout named `.md`: no magic at 0x100 → not a ROM (use `.smd`).
    let smd = raw_to_smd(&mega_drive(Some(0x100)));
    assert_eq!(
        head("md", &smd),
        Err(ClassifyError::NotRom("missing Mega Drive header"))
    );
}

#[test]
fn smd_gen_and_8bit_go_by_extension() {
    let smd = raw_to_smd(&mega_drive(Some(0x100)));
    assert_eq!(head("smd", &smd).map(|s| s.id), Ok("genesis"));
    assert_eq!(head("GEN", &mega_drive(None)).map(|s| s.id), Ok("genesis"));
    let eight_bit = vec![0u8; 0x8000];
    assert_eq!(head("sms", &eight_bit).map(|s| s.id), Ok("sms"));
    assert_eq!(head("gg", &eight_bit).map(|s| s.id), Ok("gg"));
    assert_eq!(head("sg", &eight_bit).map(|s| s.id), Ok("sg1000"));
    assert_eq!(
        head("sms", &eight_bit[..1023]),
        Err(ClassifyError::OutOfSize)
    );
}

#[test]
fn markdown_readme_is_not_a_rom() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-classify-sega-")
        .tempdir()
        .unwrap();
    let readme = temp.path().join("README.md");
    let text = "# My ROM folder\n\nNotes about these games.\n".repeat(30);
    assert!(text.len() >= 512);
    std::fs::write(&readme, &text).unwrap();
    let error = classify_path(&readme, &NoDiscProbe).unwrap_err();
    assert_eq!(error, ClassifyError::NotRom("missing Mega Drive header"));
    assert_eq!(error.to_string(), "missing Mega Drive header");
    let game = temp.path().join("Sonic.md");
    std::fs::write(&game, mega_drive(Some(0x100))).unwrap();
    assert_eq!(
        classify_path(&game, &NoDiscProbe).map(|s| s.id),
        Ok("genesis")
    );
}
