//! Classifier tests for Nintendo DS files: header probe reasons, size, files on disk.

use super::*;
use crate::nds_header_probe::test_header::{fix_crc, nds_rom};
use crate::{NoDiscProbe, NDS_DSI_ONLY};

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

#[test]
fn valid_headers_classify_as_nds_for_every_extension() {
    let rom = nds_rom(0x2000, b"ZNSA", 0);
    for ext in ["nds", "NDS", "dsi", "ids"] {
        assert_eq!(head(ext, &rom).map(|s| s.id), Ok("nds"), "{ext}");
    }
    let enhanced = nds_rom(0x2000, b"ZNSB", 2);
    assert_eq!(head("nds", &enhanced).map(|s| s.id), Ok("nds"));
}

#[test]
fn broken_headers_report_a_reason() {
    let mut broken_crc = nds_rom(0x2000, b"ZNSA", 0);
    broken_crc[0x15E] ^= 0xFF;
    assert_eq!(
        head("nds", &broken_crc),
        Err(ClassifyError::ProbeFailed("DS header checksum mismatch"))
    );
    assert_eq!(
        head("nds", &nds_rom(0x2000, b"ZNSA", 3)),
        Err(ClassifyError::ProbeFailed(NDS_DSI_ONLY))
    );
    let mut far_entry = nds_rom(0x2000, b"ZNSA", 0);
    far_entry[0x24..0x28].copy_from_slice(&0x0800_0000u32.to_le_bytes());
    fix_crc(&mut far_entry);
    assert_eq!(
        head("nds", &far_entry),
        Err(ClassifyError::ProbeFailed("invalid DS header"))
    );
    // A GBA ROM renamed `.nds`: ARM branch at 0, no DS header CRC.
    let mut gba = vec![0u8; 0x8000];
    gba[..4].copy_from_slice(&[0x2E, 0x00, 0x00, 0xEA]);
    gba[0xA0..0xAC].copy_from_slice(b"NOSTYRA TEST");
    gba[0xB2] = 0x96;
    assert_eq!(
        head("nds", &gba),
        Err(ClassifyError::ProbeFailed("DS header checksum mismatch"))
    );
}

#[test]
fn short_files_are_out_of_size() {
    let short = nds_rom(0xFFF, b"ZNSA", 0);
    assert_eq!(head("nds", &short), Err(ClassifyError::OutOfSize));
    assert_eq!(head("nds", &short[..0x100]), Err(ClassifyError::OutOfSize));
}

#[test]
fn classify_path_reads_the_header_from_disk() {
    let dir = std::env::temp_dir().join(format!("nostyra-nds-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("Homebrew.nds");
    std::fs::write(&good, nds_rom(0x1000, b"ZNSA", 0)).unwrap();
    assert_eq!(classify_path(&good, &NoDiscProbe).map(|s| s.id), Ok("nds"));
    let dsi = dir.join("DSi Only.dsi");
    std::fs::write(&dsi, nds_rom(0x1000, b"ZNSA", 3)).unwrap();
    assert_eq!(
        classify_path(&dsi, &NoDiscProbe),
        Err(ClassifyError::ProbeFailed(NDS_DSI_ONLY))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
