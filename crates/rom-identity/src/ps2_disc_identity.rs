//! Identity of a PlayStation 2 game without reading the disc: dumps are 0.1–8.5 GB
//! and hashing them at scan or launch is out of the question, so a disc is keyed by
//! its `SYSTEM.CNF` (one ≤ 64 KiB header read, `disc_image::ps2_identity_key`):
//! serial = the `BOOT2` executable (`SLUS-20312`, matched to Redump by the 10-char
//! key), content id = SHA1 of the canonical text `"ps2:<SERIAL>"`. Every dump of one
//! serial — `.iso`, `.bin/.cue`, `.chd`, `.cso`, `.zso`, its `.m3u` — shares one
//! content id and save folder. A serial-less disc (homebrew / demos booting
//! `BOOT.ELF`) hashes `"ps2:vol:<VOLUME ID>/<BOOT2 path>"` instead and matches no
//! DAT entry; two such discs share an id only when volume id and boot path agree.

use std::path::Path;

use disc_image::{identity_source, open_data_track, ps2_identity_key, DiscError, Ps2IdentityKey};
use system_catalog::{DiscSignature, PS2};

use crate::disc_game_identity::DiscIdentity;
use crate::rom_file_hasher::hash_bytes;

/// Shown when a file claimed as PlayStation 2 has no `BOOT2` any more (it changed
/// between classification and identification).
const NO_BOOT2: &str = "not a PlayStation 2 disc";

/// Identity of the PS2 `disc` (an `.m3u` is already resolved to its first disc).
pub fn read_ps2_identity(disc: &Path) -> Result<DiscIdentity, DiscError> {
    let key = open_data_track(disc)
        .and_then(|mut reader| ps2_identity_key(reader.as_mut()))?
        .ok_or(DiscError::Format(NO_BOOT2))?;
    // The Redump-listed file (track 1 of a `.cue`, else the image) keeps the DAT
    // size tie-break meaningful for plain dumps; it is sized, never read.
    let size = content_file_source::len(&identity_source(disc, DiscSignature::PlayStation2)?)?;
    Ok(DiscIdentity {
        hashes: hash_bytes(ps2_disc_identity(&key).as_bytes(), size),
        serial: key.serial().map(str::to_owned),
        title: None,
    })
}

/// The canonical text a PS2 content id hashes: `"ps2:SLUS-20312"`, or
/// `"ps2:vol:NOSTYRA_HB/BOOT.ELF;1"` for a serial-less disc.
pub fn ps2_disc_identity(key: &Ps2IdentityKey) -> String {
    match key {
        Ps2IdentityKey::Serial(serial) => format!("{}:{serial}", PS2.id),
        Ps2IdentityKey::VolumeExe { volume_id, exe } => {
            format!("{}:vol:{volume_id}/{exe}", PS2.id)
        }
    }
}

#[cfg(test)]
mod tests {
    use disc_image::test_images::{to_chd, to_iso, to_raw, write, CountingReader};
    use disc_image::test_ps2_images::{ps2_sectors, ps2_volume, zso, TEST_EXE, TEST_SERIAL};
    use disc_image::test_psp_containers::{cso, dvd_chd};

    use super::*;
    use crate::disc_game_identity::read_disc_identity;

    fn sha1_of(text: &str) -> String {
        hash_bytes(text.as_bytes(), 0).sha1
    }

    #[test]
    fn every_container_of_one_disc_shares_the_serial_content_id() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let sectors = ps2_sectors(TEST_EXE);
        let iso = to_iso(&sectors);
        let raw = to_raw(&sectors, 2);
        write(d, "g.bin", &raw);
        let cue = b"FILE \"g.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let files = [
            write(d, "g.iso", &iso),
            write(d, "g.cue", cue),
            write(d, "dvd.chd", &dvd_chd(&iso, true)),
            write(d, "cd.chd", &to_chd(&raw, "MODE2_RAW")),
            write(d, "g.cso", &cso(&iso, 2048, 0)),
            write(d, "g.zso", &zso(&iso, 2048, 1)),
            write(d, "set.m3u", b"g.cue\ng.iso\n"),
        ];
        let want = sha1_of(&format!("ps2:{TEST_SERIAL}"));
        for file in &files {
            let id = read_disc_identity(file, DiscSignature::PlayStation2).unwrap();
            assert_eq!(id.hashes.sha1, want, "{}", file.display());
            assert_eq!(id.serial.as_deref(), Some(TEST_SERIAL));
            assert_eq!(id.title, None);
        }
        // The DAT tie-break size is the Redump-listed file: the `.iso` itself, the
        // `.bin` behind a `.cue` / `.m3u`.
        let size = |name: &str| {
            read_disc_identity(&d.join(name), DiscSignature::PlayStation2)
                .unwrap()
                .hashes
                .size
        };
        assert_eq!(size("g.iso"), iso.len() as u64);
        assert_eq!(size("g.cue"), raw.len() as u64);
        assert_eq!(size("set.m3u"), raw.len() as u64);
    }

    #[test]
    fn serial_less_discs_hash_the_volume_id_and_boot_path() {
        let dir = tempfile::tempdir().unwrap();
        let cnf = b"BOOT2 = cdrom0:\\BOOT.ELF;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
        let hb = |volume: &str| {
            let sectors = ps2_volume(volume, &[("SYSTEM.CNF", cnf), ("BOOT.ELF", &[7; 48])]);
            write(dir.path(), &format!("{volume}.iso"), &to_iso(&sectors))
        };
        let a = read_disc_identity(&hb("NOSTYRA_HB"), DiscSignature::PlayStation2).unwrap();
        assert_eq!(a.hashes.sha1, sha1_of("ps2:vol:NOSTYRA_HB/BOOT.ELF;1"));
        assert_eq!(a.serial, None, "no DAT serial match for homebrew");
        let b = read_disc_identity(&hb("OTHER_HB"), DiscSignature::PlayStation2).unwrap();
        assert_ne!(a.hashes.sha1, b.hashes.sha1);
        assert_eq!(
            ps2_disc_identity(&Ps2IdentityKey::Serial("SLPS-25497".into())),
            "ps2:SLPS-25497"
        );
    }

    #[test]
    fn identity_reads_the_header_only_and_rejects_non_ps2_volumes() {
        let dir = tempfile::tempdir().unwrap();
        let iso = write(dir.path(), "g.iso", &to_iso(&ps2_sectors(TEST_EXE)));
        let mut file = open_data_track(&iso).unwrap();
        let mut counting = CountingReader {
            inner: file.as_mut(),
            bytes: 0,
        };
        let key = ps2_identity_key(&mut counting).unwrap().unwrap();
        assert_eq!(key, Ps2IdentityKey::Serial(TEST_SERIAL.into()));
        assert!(counting.bytes <= 64 * 1024, "{} bytes", counting.bytes);
        let sectors = disc_image::test_images::iso_sectors("PLAYSTATION", None, &[]);
        let plain = write(dir.path(), "plain.iso", &to_iso(&sectors));
        let err = read_disc_identity(&plain, DiscSignature::PlayStation2).unwrap_err();
        assert_eq!(err.to_string(), NO_BOOT2);
    }
}

/// Dev-only: `cargo test -p rom-identity -- --ignored --nocapture` with the
/// developer's disc in `E:/PS2` (read-only, never a fixture).
#[cfg(test)]
#[test]
#[ignore = "needs the developer's own PS2 disc in E:/PS2"]
fn developer_disc_identity_is_the_serial_hash() {
    let path = Path::new("E:/PS2/Berwick Saga.iso");
    let identity =
        crate::disc_game_identity::read_disc_identity(path, DiscSignature::PlayStation2).unwrap();
    println!("{identity:?}");
    assert_eq!(identity.serial.as_deref(), Some("SLPS-25497"));
    assert_eq!(identity.hashes.sha1, hash_bytes(b"ps2:SLPS-25497", 0).sha1);
    assert_eq!(identity.hashes.size, 4_321_804_288);
}
