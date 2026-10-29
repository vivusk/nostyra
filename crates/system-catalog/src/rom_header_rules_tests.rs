//! Probe and header-rule unit tests for `rom_header_rules`.

use super::*;

const NES_MAGIC: ContentProbe = ContentProbe::Magic {
    offset: 0,
    bytes: b"NES\x1A",
    missing: "missing iNES header",
};

#[test]
fn magic_probe_accepts_and_rejects() {
    assert_eq!(NES_MAGIC.check("nes", b"NES\x1A\x02\x01"), Ok(()));
    assert_eq!(NES_MAGIC.check("nes", b"NES"), Err("missing iNES header"));
    assert_eq!(
        NES_MAGIC.check("nes", b"\0\0\0\0"),
        Err("missing iNES header")
    );
    assert_eq!(ContentProbe::Any.check("sfc", b""), Ok(()));
    assert!(NES_MAGIC.needs_bytes("nes"));
    assert!(!ContentProbe::Any.needs_bytes("sfc"));
    let disc = ContentProbe::Disc(DiscSignature::PlayStation);
    assert!(!disc.needs_bytes("bin"));
    assert!(disc.checks_content("bin") && NES_MAGIC.checks_content("nes"));
    assert!(!ContentProbe::Any.checks_content("sfc"));
    assert!(disc.check("bin", b"anything").is_err());
    assert!(!NES_MAGIC.is_quiet("nes") && !disc.is_quiet("bin"));
}

const SEGA_PROBE: ContentProbe = ContentProbe::MagicForExtensions {
    exts: &["bin"],
    quiet_exts: &["md"],
    offsets: &[0x100, 0x101],
    bytes: b"SEGA",
    missing: "missing Mega Drive header",
};

#[test]
fn magic_for_extensions_checks_listed_extensions_only() {
    let mut at_100 = vec![0u8; 0x110];
    at_100[0x100..0x104].copy_from_slice(b"SEGA");
    let mut at_101 = vec![0u8; 0x110];
    at_101[0x101..0x105].copy_from_slice(b"SEGA");
    let blank = vec![0u8; 0x110];
    let missing = Err("missing Mega Drive header");
    for ext in ["bin", "BIN", "md", "Md"] {
        assert!(
            SEGA_PROBE.checks_content(ext) && SEGA_PROBE.needs_bytes(ext),
            "{ext}"
        );
        assert_eq!(SEGA_PROBE.check(ext, &at_100), Ok(()));
        assert_eq!(SEGA_PROBE.check(ext, &at_101), Ok(()));
        assert_eq!(SEGA_PROBE.check(ext, &blank), missing);
        assert_eq!(SEGA_PROBE.check(ext, &at_100[..0x103]), missing);
    }
    // Other extensions of the system: the extension decides, nothing is read.
    for ext in ["gen", "smd"] {
        assert!(!SEGA_PROBE.checks_content(ext) && !SEGA_PROBE.needs_bytes(ext));
        assert_eq!(SEGA_PROBE.check(ext, &blank), Ok(()));
    }
    assert!(SEGA_PROBE.is_quiet("MD") && !SEGA_PROBE.is_quiet("bin"));
}

#[test]
fn hash_skip_len_per_rule() {
    assert_eq!(hash_skip_len(HeaderRule::SnesCopier512, 1_048_576), 0);
    assert_eq!(
        hash_skip_len(HeaderRule::SnesCopier512, 1_048_576 + 512),
        512
    );
    // NES: the whole file is hashed, whatever the size.
    assert_eq!(hash_skip_len(HeaderRule::INes, 40_976), 0);
    assert_eq!(hash_skip_len(HeaderRule::INes, 1024 + 512), 0);
    assert_eq!(hash_skip_len(HeaderRule::None, 1024 + 512), 0);
    // Mega Drive: 512 for the SMD size (n * 16 KiB + 512), else the whole file.
    assert_eq!(hash_skip_len(HeaderRule::MegaDriveSmd, 0x20000 + 512), 512);
    assert_eq!(hash_skip_len(HeaderRule::MegaDriveSmd, 0x20000), 0);
    assert_eq!(hash_skip_len(HeaderRule::MegaDriveSmd, 1024 + 512), 0);
}

#[test]
fn ines_header_with_and_without_trainer() {
    let mut header = *b"NES\x1A\x02\x01\x00\x00\0\0\0\0\0\0\0\0";
    assert_eq!(ines_header_len(&header), 16);
    header[6] = 0x04;
    assert_eq!(ines_header_len(&header), 528);
    assert_eq!(ines_header_len(&header[..15]), 0);
}
