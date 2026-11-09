//! PlayStation 2 (RA console 21): `rc_hash_ps2` of the test disc through our cdreader
//! = `md5("NSTR_000.01" + ELF bytes)` — rcheevos strips `cdrom0:\` and stops at the
//! `;` (`hash_disc.c` `rc_hash_find_playstation_executable`), so `;1` is not hashed
//! (Nostyra's own `ps2:vol:` identity key keeps it, that is a different value) — and
//! the same for the DVD CHD / CSO / ZSO twins built from the ISO in a temp dir.

use disc_image::test_images::write;
use disc_image::test_ps2_images::zso;
use disc_image::test_psp_containers::{cso, dvd_chd};

use super::tests::{default_hash, roms};
use crate::content_hash;

/// `build-nostyra-test-ps2.py` prints this: Python `hashlib.md5(b"NSTR_000.01" + elf)`
/// (with `b"NSTR_000.01;1"` it would be another value).
const PS2_HASH: &str = "11f4d27b1591e25cdb2edf0b0e0fdd6b";
const PS2: u32 = 21;

#[test]
fn test_disc_hash_is_the_executable_name_without_version_plus_its_bytes() {
    let iso = roms().join("nostyra-test-ps2.iso");
    assert_eq!(content_hash(PS2, &iso, None).as_deref(), Some(PS2_HASH));
    assert_eq!(
        default_hash(PS2, &iso).as_deref(),
        Some(PS2_HASH),
        "plain ISO: the default reader agrees"
    );
}

#[test]
fn chd_cso_and_zso_twins_hash_like_the_iso() {
    let dir = tempfile::tempdir().unwrap();
    let iso = std::fs::read(roms().join("nostyra-test-ps2.iso")).unwrap();
    for (name, bytes) in [
        ("ps2.chd", dvd_chd(&iso, true)),
        ("ps2.cso", cso(&iso, 2048, 0)),
        ("ps2.zso", zso(&iso, 2048, 1)),
    ] {
        let path = write(dir.path(), name, &bytes);
        assert_eq!(
            default_hash(PS2, &path),
            None,
            "{name}: default reader cannot"
        );
        assert_eq!(
            content_hash(PS2, &path, None).as_deref(),
            Some(PS2_HASH),
            "{name}"
        );
    }
}
