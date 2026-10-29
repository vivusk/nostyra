//! Nintendo DS cartridge header check ([`ContentProbe::NdsHeader`](crate::ContentProbe)):
//! the header CRC plus the fields melonDS validates, read from the first 0x160
//! bytes. The Nintendo logo bytes are never compared (homebrew carries none).

/// Header bytes the probe reads: the header CRC field ends at 0x160.
pub(crate) const NDS_HEADER_PROBE_BYTES: usize = 0x160;
const _: () = assert!(NDS_HEADER_PROBE_BYTES <= crate::PROBE_BYTES);

/// Reason for a DSi-only game (unit code 3): melonDS DS plays it only in DSi mode,
/// which needs a NAND image Nostyra does not handle.
pub const NDS_DSI_ONLY: &str = "DSi-only game — not supported";

const MISSING: &str = "missing DS header";
const BAD_CRC: &str = "DS header checksum mismatch";
const INVALID: &str = "invalid DS header";

/// The DS header itself (0x200 bytes): code never starts inside it.
const HEADER_BYTES: u32 = 0x200;
/// ARM9 entry range melonDS DS accepts (main RAM minus the BIOS-reserved top).
const ARM9_ENTRY: std::ops::RangeInclusive<u32> = 0x0200_0000..=0x023B_FE00;
const UNIT_DS: u8 = 0;
const UNIT_DS_DSI: u8 = 2;
const UNIT_DSI_ONLY: u8 = 3;

/// `Ok` when `first_bytes` start with a valid DS header: CRC-16/MODBUS over
/// 0x000–0x15D equal to the u16 at 0x15E, ARM9 entry in main RAM, ARM9/ARM7 ROM
/// offsets past the header, unit code 0 (DS) or 2 (DS + DSi). Unit code 3 fails
/// with [`NDS_DSI_ONLY`].
pub(crate) fn check_nds_header(first_bytes: &[u8]) -> Result<(), &'static str> {
    let Some(header) = first_bytes.get(..NDS_HEADER_PROBE_BYTES) else {
        return Err(MISSING);
    };
    if crc16_modbus(&header[..0x15E]) != u16::from_le_bytes([header[0x15E], header[0x15F]]) {
        return Err(BAD_CRC);
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };
    let (arm9_offset, arm9_entry, arm7_offset) = (word(0x20), word(0x24), word(0x30));
    if !ARM9_ENTRY.contains(&arm9_entry) || arm9_offset < HEADER_BYTES || arm7_offset < HEADER_BYTES
    {
        return Err(INVALID);
    }
    match header[0x12] {
        UNIT_DS | UNIT_DS_DSI => Ok(()),
        UNIT_DSI_ONLY => Err(NDS_DSI_ONLY),
        _ => Err(INVALID),
    }
}

/// CRC-16/MODBUS (poly 0xA001 reflected, init 0xFFFF): the DS header CRC.
pub fn crc16_modbus(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0xFFFF, |crc, &byte| {
        (0..8).fold(crc ^ u16::from(byte), |crc, _| {
            if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            }
        })
    })
}

#[cfg(test)]
pub(crate) mod test_header {
    use super::crc16_modbus;

    /// A minimal homebrew-style header (`size` bytes): game code `code`, unit code
    /// `unit`, ARM9 at 0x200 → 0x02000000, ARM7 at 0x400, valid header CRC.
    pub(crate) fn nds_rom(size: usize, code: &[u8; 4], unit: u8) -> Vec<u8> {
        let mut rom = vec![0u8; size];
        rom[..8].copy_from_slice(b"NOSTYRA\0");
        rom[0x0C..0x10].copy_from_slice(code);
        rom[0x12] = unit;
        let mut put = |offset: usize, value: u32| {
            rom[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(0x20, 0x200);
        put(0x24, 0x0200_0000);
        put(0x28, 0x0200_0000);
        put(0x30, 0x400);
        put(0x34, 0x037F_8000);
        fix_crc(&mut rom);
        rom
    }

    /// Rewrites the header CRC at 0x15E after a field change.
    pub(crate) fn fix_crc(rom: &mut [u8]) {
        let crc = crc16_modbus(&rom[..0x15E]);
        rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::test_header::{fix_crc, nds_rom};
    use super::*;

    #[test]
    fn crc16_modbus_check_value() {
        assert_eq!(crc16_modbus(b"123456789"), 0x4B37);
    }

    #[test]
    fn valid_homebrew_and_enhanced_headers_pass() {
        assert_eq!(check_nds_header(&nds_rom(0x1000, b"ZNSA", 0)), Ok(()));
        assert_eq!(check_nds_header(&nds_rom(0x1000, b"####", 2)), Ok(()));
    }

    #[test]
    fn broken_fields_fail_with_a_reason() {
        let mut rom = nds_rom(0x1000, b"ZNSA", 0);
        rom[0x0C] = b'Y';
        assert_eq!(
            check_nds_header(&rom),
            Err(BAD_CRC),
            "field changed, CRC not"
        );
        assert_eq!(
            check_nds_header(&nds_rom(0x1000, b"ZNSA", 3)),
            Err(NDS_DSI_ONLY)
        );
        assert_eq!(check_nds_header(&nds_rom(0x1000, b"ZNSA", 1)), Err(INVALID));
        for (offset, value) in [
            (0x24, 0x0100_0000u32),
            (0x24, 0x023B_FE04),
            (0x20, 0x100),
            (0x30, 0),
        ] {
            let mut rom = nds_rom(0x1000, b"ZNSA", 0);
            rom[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            fix_crc(&mut rom);
            assert_eq!(
                check_nds_header(&rom),
                Err(INVALID),
                "{offset:#x}={value:#x}"
            );
        }
        assert_eq!(check_nds_header(&rom[..0x15F]), Err(MISSING), "short");
        assert_eq!(check_nds_header(&[0; 0x200]), Err(BAD_CRC), "zeros");
    }
}
