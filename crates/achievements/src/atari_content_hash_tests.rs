//! RetroAchievements hashes of the Atari test ROMs (M17): 2600 and a BS93 `.o` hash
//! the whole file; rcheevos strips the `.a78` (128 B) and `.lnx` (64 B) headers, so a
//! headered dump hashes like its headerless twin. Reference: Python `hashlib.md5`.

use super::tests::{default_hash, roms};
use super::*;

const ATARI: &[(&str, u32, &str)] = &[
    ("nostyra-test.a26", 25, "eb78c3674c8ee1608061a27bea3920d2"),
    ("nostyra-test.a78", 51, "8d1bcd89f22d9385abeef201a65ff978"),
    (
        "nostyra-test-7800.bin",
        51,
        "8d1bcd89f22d9385abeef201a65ff978",
    ),
    ("nostyra-test.o", 13, "e23d500fb8921a5e2bf9880a4cc7cbfd"),
    ("nostyra-test.lnx", 13, "4e00fff931deb84e5d748f21d5071548"),
];

#[test]
fn atari_hashes_match_the_python_reference_and_strip_headers() {
    for (name, console, reference) in ATARI {
        let path = roms().join(name);
        assert_eq!(
            default_hash(*console, &path).as_deref(),
            Some(*reference),
            "{name} default"
        );
        assert_eq!(
            content_hash(*console, &path, None).as_deref(),
            Some(*reference),
            "{name}"
        );
    }
}
