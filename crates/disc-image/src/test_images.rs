//! Synthetic disc images for tests: a tiny ISO9660 volume (PVD, root dir, files),
//! wrapped as 2048 ISO, raw MODE1/MODE2 2352, or an uncompressed CHD v5.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use crate::disc_sector_reader::{DiscReader, SectorData, SECTOR_DATA_BYTES};
use crate::DiscError;

pub const US_LICENCE: &str =
    "          Licensed  by          Sony Computer Entertainment Amer  ica ";
pub const JP_LICENCE: &str = "          Licensed  by          Sony Computer Entertainment Inc.";
pub const EU_LICENCE: &str =
    "          Licensed  by          Sony Computer Entertainment Euro pe   ";

/// 2048-byte sectors of a volume holding `files` in its root directory (sector 18),
/// file data from sector 19. `licence` goes to sector 4 (at byte 32, like real discs).
pub fn iso_sectors(
    system_id: &str,
    licence: Option<&str>,
    files: &[(&str, &[u8])],
) -> Vec<Vec<u8>> {
    let mut sectors = vec![vec![0u8; SECTOR_DATA_BYTES]; 19];
    if let Some(text) = licence {
        sectors[4][32..32 + text.len()].copy_from_slice(text.as_bytes());
    }
    let mut root = Vec::new();
    root.extend(dir_record(18, 2048, &[0]));
    root.extend(dir_record(18, 2048, &[1]));
    let mut next_lba = 19u32;
    for (name, data) in files {
        root.extend(dir_record(
            next_lba,
            data.len() as u32,
            format!("{name};1").as_bytes(),
        ));
        for chunk in data.chunks(SECTOR_DATA_BYTES) {
            let mut sector = chunk.to_vec();
            sector.resize(SECTOR_DATA_BYTES, 0);
            sectors.push(sector);
            next_lba += 1;
        }
    }
    root.resize(SECTOR_DATA_BYTES, 0);
    sectors.push(vec![0; SECTOR_DATA_BYTES]); // spare tail sector
    let pvd = &mut sectors[16];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[8..40].copy_from_slice(format!("{system_id:<32}").as_bytes());
    pvd[40..72].copy_from_slice(format!("{:<32}", "NOSTYRA_TEST").as_bytes());
    pvd[156..190].copy_from_slice(&dir_record(18, 2048, &[0])[..34]);
    sectors[17][0] = 255; // volume descriptor set terminator
    sectors[17][1..6].copy_from_slice(b"CD001");
    sectors[18] = root;
    sectors
}

/// A PS1-like volume whose SYSTEM.CNF has `boot_line`, plus `SLUS_012.34`: a
/// minimal PS-X EXE (spins forever at 0x80010000) so PCSX ReARMed boots it.
pub fn ps1_sectors(licence: Option<&str>, boot_line: &str) -> Vec<Vec<u8>> {
    let cnf = format!("{boot_line}\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801FFFF0\r\n");
    iso_sectors(
        "PLAYSTATION",
        licence,
        &[
            ("SYSTEM.CNF", cnf.as_bytes()),
            ("SLUS_012.34", &spinning_exe()),
        ],
    )
}

/// PS-X EXE: 2048-byte header + one 2048-byte text block holding `j 0x80010000; nop`.
fn spinning_exe() -> Vec<u8> {
    const LOAD: u32 = 0x8001_0000;
    let mut exe = vec![0u8; 4096];
    exe[..8].copy_from_slice(b"PS-X EXE");
    exe[0x10..0x14].copy_from_slice(&LOAD.to_le_bytes()); // pc0
    exe[0x18..0x1C].copy_from_slice(&LOAD.to_le_bytes()); // t_addr
    exe[0x1C..0x20].copy_from_slice(&2048u32.to_le_bytes()); // t_size
    exe[0x30..0x34].copy_from_slice(&0x801F_FFF0u32.to_le_bytes()); // s_addr
    let jump = 0x0800_0000 | ((LOAD & 0x0FFF_FFFF) >> 2);
    exe[2048..2052].copy_from_slice(&jump.to_le_bytes());
    exe
}

fn dir_record(lba: u32, size: u32, name: &[u8]) -> Vec<u8> {
    let len = 33 + name.len() + (name.len() + 1) % 2;
    let mut r = vec![0u8; len];
    r[0] = len as u8;
    r[2..6].copy_from_slice(&lba.to_le_bytes());
    r[6..10].copy_from_slice(&lba.to_be_bytes());
    r[10..14].copy_from_slice(&size.to_le_bytes());
    r[14..18].copy_from_slice(&size.to_be_bytes());
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}

pub fn to_iso(sectors: &[Vec<u8>]) -> Vec<u8> {
    sectors.concat()
}

/// Raw 2352-byte sectors: sync, BCD MSF header (+150 frames), mode, (MODE2: 8-byte
/// subheader), data, zero EDC/ECC.
pub fn to_raw(sectors: &[Vec<u8>], mode: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for (lba, data) in sectors.iter().enumerate() {
        let mut s = vec![0u8; 2352];
        s[1..11].fill(0xFF);
        let f = lba + 150;
        let bcd = |v: usize| (((v / 10) << 4) | (v % 10)) as u8;
        s[12] = bcd(f / 75 / 60);
        s[13] = bcd(f / 75 % 60);
        s[14] = bcd(f % 75);
        s[15] = mode;
        let at = if mode == 1 { 16 } else { 24 };
        s[at..at + 2048].copy_from_slice(data);
        out.extend(s);
    }
    out
}

/// Uncompressed CHD v5 of raw 2352-byte sectors, 8 frames per hunk, one `CHT2` track.
pub fn to_chd(raw: &[u8], track_type: &str) -> Vec<u8> {
    let text = format!(
        "TRACK:1 TYPE:{track_type} SUBTYPE:NONE FRAMES:{} PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0",
        raw.len() / 2352
    );
    crate::test_gd_images::chd_v5(raw, &[(*b"CHT2", text)])
}

pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Counts bytes handed out, to bound the I/O of an identification.
pub struct CountingReader<'a> {
    pub inner: &'a mut dyn DiscReader,
    pub bytes: usize,
}

impl DiscReader for CountingReader<'_> {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError> {
        self.bytes += SECTOR_DATA_BYTES;
        self.inner.read_user_data(lba)
    }

    fn sector_count(&self) -> u64 {
        self.inner.sector_count()
    }
}

/// `EBOOT.PBP` whose `PARAM.SFO` holds `CATEGORY` and `DISC_ID` (no disc data).
pub fn pbp_image(category: &str, disc_id: &str) -> Vec<u8> {
    let entries = [("CATEGORY", category), ("DISC_ID", disc_id)];
    let (mut keys, mut data, mut table) = (Vec::new(), Vec::new(), Vec::new());
    for (key, value) in entries {
        let len = value.len() as u32 + 1;
        table.extend((keys.len() as u16).to_le_bytes());
        table.extend(0x0204u16.to_le_bytes());
        table.extend(len.to_le_bytes());
        table.extend(len.to_le_bytes());
        table.extend((data.len() as u32).to_le_bytes());
        keys.extend(key.as_bytes());
        keys.push(0);
        data.extend(value.as_bytes());
        data.push(0);
    }
    let key_start = 20 + table.len() as u32;
    let mut sfo = b"\0PSF".to_vec();
    sfo.extend(0x0101u32.to_le_bytes());
    sfo.extend(key_start.to_le_bytes());
    sfo.extend((key_start + keys.len() as u32).to_le_bytes());
    sfo.extend((entries.len() as u32).to_le_bytes());
    sfo.extend(table);
    sfo.extend(keys);
    sfo.extend(data);
    let mut pbp = b"\0PBP".to_vec();
    pbp.extend(0x0001_0000u32.to_le_bytes());
    pbp.extend(0x28u32.to_le_bytes());
    pbp.extend((0x28 + sfo.len() as u32).to_le_bytes());
    pbp.resize(0x28, 0);
    pbp.extend(sfo);
    pbp.extend([0u8; 64]);
    pbp
}
