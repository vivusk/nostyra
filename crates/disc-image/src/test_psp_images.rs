//! Synthetic PSP images for tests: `PARAM.SFO` tables and an
//! ISO9660 volume with a directory tree (`PSP_GAME/SYSDIR/…`). Containers (PBP, CSO,
//! DVD CHD) are in `test_psp_containers`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::collections::BTreeMap;

use crate::disc_sector_reader::SECTOR_DATA_BYTES;

/// Serial of the test disc as `UMD_DATA.BIN` / `PARAM.SFO` store it.
pub const TEST_DISC_ID: &str = "NSTY00001";
pub const TEST_TITLE: &str = "Nostyra Test PSP";

/// One `PARAM.SFO` value.
#[derive(Clone, Copy)]
pub enum SfoValue<'a> {
    Str(&'a str),
    U32(u32),
}

/// A `PARAM.SFO` table (`\0PSF` v1.1) holding `entries` in order.
pub fn sfo(entries: &[(&str, SfoValue)]) -> Vec<u8> {
    let (mut keys, mut data, mut table) = (Vec::new(), Vec::new(), Vec::new());
    for (key, value) in entries {
        let (fmt, bytes) = match value {
            SfoValue::Str(text) => (0x0204u16, [text.as_bytes(), &[0]].concat()),
            SfoValue::U32(n) => (0x0404u16, n.to_le_bytes().to_vec()),
        };
        let max = bytes.len().next_multiple_of(4) as u32;
        table.extend((keys.len() as u16).to_le_bytes());
        table.extend(fmt.to_le_bytes());
        table.extend((bytes.len() as u32).to_le_bytes());
        table.extend(max.to_le_bytes());
        table.extend((data.len() as u32).to_le_bytes());
        keys.extend(key.as_bytes());
        keys.push(0);
        data.extend(&bytes);
        data.resize(data.len() + max as usize - bytes.len(), 0);
    }
    keys.resize(keys.len().next_multiple_of(4), 0);
    let key_start = 0x14 + table.len() as u32;
    let mut out = b"\0PSF".to_vec();
    out.extend(0x0101u32.to_le_bytes());
    out.extend(key_start.to_le_bytes());
    out.extend((key_start + keys.len() as u32).to_le_bytes());
    out.extend((entries.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(keys);
    out.extend(data);
    out
}

/// `PARAM.SFO` of a PSP game: `category`, the test serial, title and version.
pub fn game_sfo(category: &str) -> Vec<u8> {
    sfo(&[
        ("CATEGORY", SfoValue::Str(category)),
        ("DISC_ID", SfoValue::Str(TEST_DISC_ID)),
        ("DISC_VERSION", SfoValue::Str("1.00")),
        ("PARENTAL_LEVEL", SfoValue::U32(1)),
        ("TITLE", SfoValue::Str(TEST_TITLE)),
    ])
}

/// 2048-byte sectors of an ISO9660 volume with system id `system_id` holding
/// `files` by path (`PSP_GAME/PARAM.SFO`); directories are created as needed, a
/// directory with many entries spans several sectors.
pub fn iso_tree(system_id: &str, files: &[(&str, &[u8])]) -> Vec<Vec<u8>> {
    // Directory path ("" = root) → children (name, is_dir, file index).
    let mut dirs: BTreeMap<String, Vec<(String, Option<usize>)>> = BTreeMap::new();
    dirs.insert(String::new(), Vec::new());
    for (index, (path, _)) in files.iter().enumerate() {
        let parts: Vec<&str> = path.split('/').collect();
        for depth in 0..parts.len() {
            let parent = parts[..depth].join("/");
            let is_file = depth + 1 == parts.len();
            let child = (parts[depth].to_owned(), is_file.then_some(index));
            let siblings = dirs.entry(parent.clone()).or_default();
            if !siblings.contains(&child) {
                siblings.push(child);
            }
            if !is_file {
                dirs.entry(join(&parent, parts[depth])).or_default();
            }
        }
    }
    let record_len = |name: &str| 33 + name.len() + (name.len() + 1) % 2;
    let dir_sectors = |children: &[(String, Option<usize>)]| {
        let lens = [34, 34].into_iter().chain(children.iter().map(|(n, f)| {
            record_len(&if f.is_some() {
                format!("{n};1")
            } else {
                n.clone()
            })
        }));
        layout(lens).1
    };
    let mut next = 18u32;
    let mut dir_at = BTreeMap::new();
    for (path, children) in &dirs {
        let sectors = dir_sectors(children) as u32;
        dir_at.insert(path.clone(), (next, sectors * 2048));
        next += sectors;
    }
    let mut file_at = Vec::new();
    for (_, data) in files {
        file_at.push((next, data.len() as u32));
        next += data.len().div_ceil(SECTOR_DATA_BYTES).max(1) as u32;
    }
    let mut sectors = vec![vec![0u8; SECTOR_DATA_BYTES]; next as usize + 1];
    for (path, children) in &dirs {
        let (lba, size) = dir_at[path];
        let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
        let mut records = vec![
            dir_record(lba, size, &[0], true),
            dir_record(dir_at[parent].0, dir_at[parent].1, &[1], true),
        ];
        for (name, file) in children {
            records.push(match file {
                Some(i) => {
                    let (l, s) = file_at[*i];
                    dir_record(l, s, format!("{name};1").as_bytes(), false)
                }
                None => {
                    let (l, s) = dir_at[&join(path, name)];
                    dir_record(l, s, name.as_bytes(), true)
                }
            });
        }
        let (starts, _) = layout(records.iter().map(Vec::len));
        for (record, (sector, at)) in records.iter().zip(starts) {
            sectors[lba as usize + sector][at..at + record.len()].copy_from_slice(record);
        }
    }
    for ((lba, _), (_, data)) in file_at.iter().zip(files) {
        for (i, chunk) in data.chunks(SECTOR_DATA_BYTES).enumerate() {
            sectors[*lba as usize + i][..chunk.len()].copy_from_slice(chunk);
        }
    }
    let pvd = &mut sectors[16];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[8..40].copy_from_slice(format!("{system_id:<32}").as_bytes());
    pvd[40..72].copy_from_slice(format!("{:<32}", "NOSTYRA_TEST_PSP").as_bytes());
    pvd[156..190].copy_from_slice(&dir_record(18, dir_at[""].1, &[0], true)[..34]);
    sectors[17][0] = 255;
    sectors[17][1..6].copy_from_slice(b"CD001");
    sectors
}

/// The test PSP disc: `UMD_DATA.BIN`, `PSP_GAME/PARAM.SFO`, `PSP_GAME/SYSDIR/EBOOT.BIN`.
pub fn psp_iso_sectors() -> Vec<Vec<u8>> {
    let umd = format!("{}|0000000000000000|0001|G", "NSTY-00001");
    iso_tree(
        "PSP GAME",
        &[
            ("UMD_DATA.BIN", umd.as_bytes()),
            ("PSP_GAME/PARAM.SFO", &game_sfo("UG")),
            ("PSP_GAME/SYSDIR/EBOOT.BIN", b"\x7fELF"),
        ],
    )
}

/// (sector, byte offset) of each record packed into 2048-byte sectors (records
/// never cross a sector), and the sector count.
fn layout(lens: impl Iterator<Item = usize>) -> (Vec<(usize, usize)>, usize) {
    let (mut starts, mut sector, mut used) = (Vec::new(), 0, 0);
    for len in lens {
        if used + len > SECTOR_DATA_BYTES {
            sector += 1;
            used = 0;
        }
        starts.push((sector, used));
        used += len;
    }
    (starts, sector + 1)
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

fn dir_record(lba: u32, size: u32, name: &[u8], dir: bool) -> Vec<u8> {
    let len = 33 + name.len() + (name.len() + 1) % 2;
    let mut r = vec![0u8; len];
    r[0] = len as u8;
    r[2..6].copy_from_slice(&lba.to_le_bytes());
    r[6..10].copy_from_slice(&lba.to_be_bytes());
    r[10..14].copy_from_slice(&size.to_le_bytes());
    r[14..18].copy_from_slice(&size.to_be_bytes());
    r[25] = if dir { 2 } else { 0 };
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}
