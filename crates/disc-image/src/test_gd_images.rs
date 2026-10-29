//! Synthetic Dreamcast images for tests: IP.BIN sectors wrapped as a TOSEC `.gdi`
//! set, a Redump density-area `.cue`, a GD-ROM CHD (`CHGD` metadata) and DiscJuggler
//! `.cdi` images (v2 / v3 / v3.5). Plus the shared uncompressed CHD v5 writer.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use crate::disc_sector_reader::SECTOR_DATA_BYTES;
use crate::test_images::{to_raw, write};

/// IP.BIN sector 0 (2048 bytes) for `product` / `title`, area `JUE`.
pub fn ip_bin_sector(product: &str, title: &str) -> Vec<u8> {
    let mut s = vec![b' '; SECTOR_DATA_BYTES];
    let mut put = |at: usize, text: &str| s[at..at + text.len()].copy_from_slice(text.as_bytes());
    put(0x00, "SEGA SEGAKATANA SEGA ENTERPRISES");
    put(0x20, "0000 GD-ROM1/1  JUE     E000F10 ");
    put(0x40, &format!("{product:<10}"));
    put(0x4A, "V1.000");
    put(0x60, "1ST_READ.BIN    ");
    put(0x80, title);
    s[0x100..].fill(0);
    s
}

/// The high-density data track: IP.BIN then `extra` zero sectors.
pub fn hd_track_sectors(product: &str, extra: usize) -> Vec<Vec<u8>> {
    let mut sectors = vec![ip_bin_sector(product, "NOSTYRA TEST")];
    sectors.extend(std::iter::repeat_n(vec![0; SECTOR_DATA_BYTES], extra));
    sectors
}

/// TOSEC-style 3-track GDI in `dir`: track01 data (4 raw sectors), track02 audio,
/// track03 raw MODE1 at LBA 45000 holding IP.BIN (the largest file). Returns the
/// sheet path; `quoted` writes names with spaces in quotes.
pub fn write_gdi(dir: &Path, stem: &str, product: &str, quoted: bool) -> PathBuf {
    let name = |n: u32, ext: &str| {
        if quoted {
            format!("{stem} track{n:02}.{ext}")
        } else {
            format!("{stem}-track{n:02}.{ext}")
        }
    };
    let t1 = to_raw(&vec![vec![0u8; SECTOR_DATA_BYTES]; 4], 1);
    let t3 = to_raw(&hd_track_sectors(product, 8), 1);
    write(dir, &name(1, "bin"), &t1);
    write(dir, &name(2, "raw"), &vec![0u8; 2352 * 2]);
    write(dir, &name(3, "bin"), &t3);
    let q = |s: String| if quoted { format!("\"{s}\"") } else { s };
    let sheet = format!(
        "3\r\n1 0 4 2352 {} 0\r\n2  154 0 2352 {} 0\r\n3 45000 4 2352 {} 0\r\n",
        q(name(1, "bin")),
        q(name(2, "raw")),
        q(name(3, "bin"))
    );
    write(dir, &format!("{stem}.gdi"), sheet.as_bytes())
}

/// Redump-style cue: Track 1 data + Track 2 audio (single density), Track 3 data with
/// IP.BIN (high density, largest file).
pub fn write_redump_gd_cue(dir: &Path, stem: &str, product: &str) -> PathBuf {
    let t1 = to_raw(&vec![vec![0u8; SECTOR_DATA_BYTES]; 4], 1);
    write(dir, &format!("{stem} (Track 1).bin"), &t1);
    write(dir, &format!("{stem} (Track 2).bin"), &vec![0u8; 2352 * 2]);
    write(
        dir,
        &format!("{stem} (Track 3).bin"),
        &to_raw(&hd_track_sectors(product, 8), 1),
    );
    let cue = format!(
        "REM SINGLE-DENSITY AREA\r\nFILE \"{stem} (Track 1).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\n    INDEX 01 00:00:00\r\nFILE \"{stem} (Track 2).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n    INDEX 01 00:00:00\r\nREM HIGH-DENSITY AREA\r\nFILE \"{stem} (Track 3).bin\" BINARY\r\n  TRACK 03 MODE1/2352\r\n    INDEX 01 00:00:00\r\n"
    );
    write(dir, &format!("{stem}.cue"), cue.as_bytes())
}

/// GD-ROM CHD: track 1 data (5 sectors, so track 2 starts after padding), track 2
/// audio, track 3 raw MODE1 with IP.BIN; one `CHGD` entry per track.
pub fn gd_chd(product: &str) -> Vec<u8> {
    let tracks: [(&str, Vec<u8>); 3] = [
        (
            "MODE1_RAW",
            to_raw(&vec![vec![0u8; SECTOR_DATA_BYTES]; 5], 1),
        ),
        ("AUDIO", vec![0u8; 2352 * 3]),
        ("MODE1_RAW", to_raw(&hd_track_sectors(product, 3), 1)),
    ];
    let mut raw = Vec::new();
    let mut metas = Vec::new();
    for (n, (kind, data)) in tracks.iter().enumerate() {
        let frames = data.len() / 2352;
        raw.extend_from_slice(data);
        raw.resize(
            raw.len() + frames.next_multiple_of(4).saturating_sub(frames) * 2352,
            0,
        );
        metas.push((
            *b"CHGD",
            format!(
                "TRACK:{} TYPE:{kind} SUBTYPE:NONE FRAMES:{frames} PAD:0 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0",
                n + 1
            ),
        ));
    }
    chd_v5(&raw, &metas)
}

/// Uncompressed CHD v5 of raw 2352-byte sectors (8 frames of 2448 bytes per hunk,
/// zero subcode) with the given metadata entries (tag, text) chained in order.
pub fn chd_v5(raw: &[u8], metas: &[([u8; 4], String)]) -> Vec<u8> {
    const FRAME: usize = 2448;
    let frames: Vec<u8> = raw
        .chunks(2352)
        .flat_map(|s| s.iter().copied().chain(std::iter::repeat_n(0, 96)))
        .collect();
    let logical = frames.len().div_ceil(FRAME * 8) * FRAME * 8;
    crate::test_psp_containers::chd_v5_units(&frames, FRAME, FRAME * 8, logical as u64, metas)
}
