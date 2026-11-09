//! Synthetic PlayStation 2 BIOS images for tests (never real BIOS bytes): a 4 MiB
//! zero-filled image whose ROMDIR table (`RESET`, `ROMDIR`, `EXTINFO`, `ROMVER`)
//! sits at a chosen offset, like the real layout (`RESET` first, table at 0x2740).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

/// Where real retail dumps keep their table (SCPH-39001 / 70012).
pub const RETAIL_TABLE_OFFSET: usize = 0x2740;

/// A 4 MiB image with `ROMVER` = `romver` (14 ASCII bytes, e.g. `0160AC20020207`)
/// reachable from a ROMDIR table at `table`.
pub fn ps2_bios_image(romver: &str, table: usize) -> Vec<u8> {
    ps2_bios_image_sized(romver, table, 4 * 1024 * 1024)
}

/// [`ps2_bios_image`] of `size` bytes.
pub fn ps2_bios_image_sized(romver: &str, table: usize, size: usize) -> Vec<u8> {
    assert_eq!(romver.len(), 14, "ROMVER is 14 bytes");
    let mut image = vec![0u8; size];
    let extinfo = 0x100usize;
    let records = [
        ("RESET", table as u32),
        ("ROMDIR", 5 * 16),
        ("EXTINFO", extinfo as u32),
        ("ROMVER", 14),
    ];
    let mut at = table;
    for (name, file_size) in records {
        write_record(&mut image, at, name, file_size);
        at += 16;
    }
    // Empty record ends the table; ROMVER's bytes follow RESET + ROMDIR + EXTINFO.
    let romver_at = table + 5 * 16 + extinfo;
    image[romver_at..romver_at + 14].copy_from_slice(romver.as_bytes());
    image
}

/// A 4 MiB image with a `RESET` record but no `ROMVER` (the user's `.ROM1` shape).
pub fn ps2_rom1_like_image() -> Vec<u8> {
    let mut image = vec![0u8; 4 * 1024 * 1024];
    write_record(&mut image, 16, "RESET", 0x1000);
    write_record(&mut image, 32, "ROMDIR", 3 * 16);
    write_record(&mut image, 48, "IOPBTCONF", 0x200);
    image
}

fn write_record(image: &mut [u8], at: usize, name: &str, file_size: u32) {
    image[at..at + name.len()].copy_from_slice(name.as_bytes());
    image[at + 12..at + 16].copy_from_slice(&file_size.to_le_bytes());
}
