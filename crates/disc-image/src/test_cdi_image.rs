//! Synthetic DiscJuggler `.cdi` images (v2 / v3 / v3.5) for tests.

use crate::disc_sector_reader::SECTOR_DATA_BYTES;
use crate::test_gd_images::hd_track_sectors;

/// DiscJuggler versions, as stored in the footer.
pub const CDI_V2: u32 = 0x8000_0004;
pub const CDI_V3: u32 = 0x8000_0005;
pub const CDI_V35: u32 = 0x8000_0006;

/// MIL-CD style DiscJuggler image: session 1 = one audio track (4 sectors), session 2
/// = one mode-2 data track (2336-byte sectors, 2-sector pregap) whose sector 0 is
/// IP.BIN. Layout mirrors Flycast's `cdipsr`.
pub fn cdi_image(version: u32, product: &str) -> Vec<u8> {
    cdi_image_with(version, &hd_track_sectors(product, 2), 11_700)
}

/// Same layout with the data track's 2048-byte `sectors`, its pregap starting at
/// `start_lba` (track LBA = `start_lba` + 2).
pub fn cdi_image_with(version: u32, sectors: &[Vec<u8>], start_lba: u32) -> Vec<u8> {
    let mut data = vec![0u8; 2352 * 4]; // audio track, no pregap
    let mut mode2 = vec![0u8; 2336 * 2]; // pregap
    for sector in sectors {
        let mut s = vec![0u8; 2336];
        s[8..8 + SECTOR_DATA_BYTES].copy_from_slice(sector);
        mode2.extend(s);
    }
    data.extend(&mode2);
    let v3 = version != CDI_V2;
    let mut table = 2u16.to_le_bytes().to_vec();
    // (mode, sector size code, pregap, length, start lba)
    let length = u32::try_from(sectors.len()).unwrap_or(u32::MAX);
    for (mode, size_code, pregap, length, lba) in
        [(0u32, 2u32, 0u32, 4u32, 0u32), (2, 1, 2, length, start_lba)]
    {
        table.extend(1u16.to_le_bytes());
        table.extend(0u32.to_le_bytes());
        let mark = [0u8, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        table.extend(mark);
        table.extend(mark);
        table.extend([0u8; 4]);
        table.push(0); // file name length
        table.extend([0u8; 19]);
        table.extend(0u32.to_le_bytes());
        table.extend([0u8; 2]);
        table.extend(pregap.to_le_bytes());
        table.extend(length.to_le_bytes());
        table.extend([0u8; 6]);
        table.extend(mode.to_le_bytes());
        table.extend([0u8; 12]);
        table.extend(lba.to_le_bytes());
        table.extend((pregap + length).to_le_bytes());
        table.extend([0u8; 16]);
        table.extend(size_code.to_le_bytes());
        table.extend([0u8; 29]);
        if v3 {
            table.extend([0u8; 5]);
            table.extend(0u32.to_le_bytes());
        }
        table.extend(vec![0u8; 12 + usize::from(v3)]);
    }
    let table_at = data.len();
    data.extend(table);
    let total = data.len() + 8;
    let offset = if version == CDI_V35 {
        total - table_at
    } else {
        table_at
    };
    data.extend(version.to_le_bytes());
    data.extend((offset as u32).to_le_bytes());
    data
}
