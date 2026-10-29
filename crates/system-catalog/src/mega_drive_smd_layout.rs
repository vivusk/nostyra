//! Mega Drive SMD (Super Magic Drive copier) layout: a 512-byte header, then the ROM
//! in 16 KiB blocks whose first half holds the odd bytes and second half the even
//! bytes. No-Intro hashes the raw image, so SMD files are converted before hashing —
//! the same rule Genesis Plus GX applies when loading (`loadrom.c`), which keeps the
//! hash, the DAT match and the content id (save folder) equal to the raw `.md` dump.

use std::borrow::Cow;

use crate::n64_byte_order::{self, N64ByteOrder};
use crate::rom_header_rules::{hash_skip_len, HeaderRule};

/// Copier header in front of the interleaved blocks.
pub(crate) const SMD_HEADER_BYTES: u64 = 512;
/// One interleaved block: 8 KiB odd bytes, then 8 KiB even bytes.
pub(crate) const SMD_BLOCK_BYTES: u64 = 0x4000;
const HALF_BLOCK: usize = 0x2000;
const HEADER: usize = SMD_HEADER_BYTES as usize;
const BLOCK: usize = SMD_BLOCK_BYTES as usize;
/// Raw dumps carry "SEGA" at 0x100 (the copier layout scrambles it).
const RAW_MAGIC_OFFSET: usize = 0x100;
const RAW_MAGIC: &[u8] = b"SEGA";

/// True for an SMD file: size `n * 16 KiB + 512` and no "SEGA" at 0x100.
pub(crate) fn is_smd_layout(bytes: &[u8]) -> bool {
    bytes.len() as u64 % SMD_BLOCK_BYTES == SMD_HEADER_BYTES
        && bytes.get(RAW_MAGIC_OFFSET..RAW_MAGIC_OFFSET + RAW_MAGIC.len()) != Some(RAW_MAGIC)
}

/// De-interleaves an SMD file (see [`is_smd_layout`]) into the raw image: per block
/// `out[2i] = block[0x2000 + i]`, `out[2i + 1] = block[i]`.
pub(crate) fn smd_to_raw(bytes: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(bytes.len().saturating_sub(HEADER));
    for block in bytes.get(HEADER..).unwrap_or_default().chunks(BLOCK) {
        let (odd, even) = block.split_at(block.len().min(HALF_BLOCK));
        for (o, e) in odd.iter().zip(even) {
            raw.extend_from_slice(&[*e, *o]);
        }
    }
    raw
}

/// The bytes a DAT hashes for a file of this system: SNES copier header skipped,
/// SMD de-interleaved, N64 `.v64`/`.n64` turned into `.z64` (a copy: callers that
/// own the buffer use [`normalise_in_place`](crate::normalise_in_place)), else the
/// file as is (borrowed, no copy).
pub fn rom_image_for_hash(rule: HeaderRule, bytes: &[u8]) -> Cow<'_, [u8]> {
    match rule {
        HeaderRule::MegaDriveSmd if is_smd_layout(bytes) => Cow::Owned(smd_to_raw(bytes)),
        HeaderRule::MegaDriveSmd => Cow::Borrowed(bytes),
        HeaderRule::N64ByteOrder => match n64_byte_order::detect(bytes) {
            None | Some(N64ByteOrder::BigEndian) => Cow::Borrowed(bytes),
            Some(_) => {
                let mut image = bytes.to_vec();
                n64_byte_order::normalise_in_place(&mut image);
                Cow::Owned(image)
            }
        },
        HeaderRule::None | HeaderRule::SnesCopier512 | HeaderRule::INes => {
            let skip = hash_skip_len(rule, bytes.len() as u64).min(bytes.len());
            Cow::Borrowed(&bytes[skip..])
        }
    }
}

/// Raw image → SMD file (inverse of [`smd_to_raw`]; header byte 0 = block count,
/// 8/9 = `0xAA 0xBB` like real copier dumps). Test helper shared by every crate's
/// SMD tests; `raw.len()` must be a multiple of 16 KiB.
#[doc(hidden)]
pub fn raw_to_smd(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; HEADER];
    out[0] = (raw.len() / BLOCK) as u8;
    out[1] = 3;
    out[8] = 0xAA;
    out[9] = 0xBB;
    for block in raw.chunks(BLOCK) {
        out.extend(block.iter().skip(1).step_by(2));
        out.extend(block.iter().step_by(2));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_rom() -> Vec<u8> {
        let mut raw: Vec<u8> = (0..2 * BLOCK).map(|i| (i * 7 + i / 251) as u8).collect();
        raw[0x100..0x104].copy_from_slice(b"SEGA");
        raw
    }

    #[test]
    fn smd_round_trip() {
        let raw = raw_rom();
        let smd = raw_to_smd(&raw);
        assert_eq!(smd.len(), raw.len() + HEADER);
        assert!(is_smd_layout(&smd));
        assert!(!is_smd_layout(&raw));
        assert_eq!(smd_to_raw(&smd), raw);
        assert_eq!(*rom_image_for_hash(HeaderRule::MegaDriveSmd, &smd), raw[..]);
    }

    #[test]
    fn raw_and_odd_sizes_hash_whole() {
        let raw = raw_rom();
        assert!(matches!(
            rom_image_for_hash(HeaderRule::MegaDriveSmd, &raw),
            Cow::Borrowed(_)
        ));
        // Right size but "SEGA" at 0x100: a raw dump, not SMD.
        let mut sized = raw.clone();
        sized.extend_from_slice(&[0; HEADER]);
        sized[0x100..0x104].copy_from_slice(b"SEGA");
        assert!(!is_smd_layout(&sized));
        assert_eq!(
            *rom_image_for_hash(HeaderRule::MegaDriveSmd, &sized),
            sized[..]
        );
        // Wrong size: whole file.
        assert!(!is_smd_layout(&raw[..raw.len() - 1]));
    }

    #[test]
    fn other_rules_skip_or_borrow() {
        let file = vec![1u8; 1024 + 512];
        assert_eq!(
            rom_image_for_hash(HeaderRule::SnesCopier512, &file).len(),
            1024
        );
        assert_eq!(
            rom_image_for_hash(HeaderRule::None, &file).len(),
            file.len()
        );
        assert_eq!(
            rom_image_for_hash(HeaderRule::INes, &file).len(),
            file.len()
        );
    }

    #[test]
    fn n64_orders_hash_as_z64() {
        let mut z64: Vec<u8> = (0..0x1000u32).map(|i| (i * 3 % 251) as u8).collect();
        z64[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
        let rule = HeaderRule::N64ByteOrder;
        assert!(matches!(rom_image_for_hash(rule, &z64), Cow::Borrowed(_)));
        for order in [N64ByteOrder::ByteSwapped, N64ByteOrder::LittleEndian] {
            let dump = crate::n64_to_order(&z64, order);
            assert_eq!(*rom_image_for_hash(rule, &dump), z64[..], "{order:?}");
        }
    }
}
