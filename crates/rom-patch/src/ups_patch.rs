//! UPS: `UPS1`, source size, target size (varints), then XOR hunks — skip count
//! (varint), XOR bytes up to a 0x00 terminator (which also consumes one position) —
//! and a CRC32 footer. Sizes and both checksums are verified.

use crate::patch_byte_reader::{checked_target_size, checksum_footer, PatchByteReader};
use crate::patch_error::PatchError;

pub(crate) fn apply(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError> {
    let footer = checksum_footer(patch)?;
    let mut reader = PatchByteReader::new(patch, 4);
    let source_size = reader.varint()?;
    let target_size = checked_target_size(reader.varint()?)?;
    if rom.len() != source_size || crc32fast::hash(rom) != footer.source {
        return Err(PatchError::SourceMismatch);
    }
    let mut out = vec![0u8; target_size];
    let common = source_size.min(target_size);
    out[..common].copy_from_slice(&rom[..common]);
    let hunks_end = patch.len() - 12;
    let mut pos = 0usize;
    while reader.pos() < hunks_end {
        pos = pos
            .checked_add(reader.varint()?)
            .ok_or(PatchError::OutOfRange)?;
        loop {
            let x = reader.byte()?;
            if x == 0 {
                pos = pos.saturating_add(1); // the terminator consumes a position too
                break;
            }
            *out.get_mut(pos).ok_or(PatchError::OutOfRange)? ^= x;
            pos += 1;
        }
    }
    if crc32fast::hash(&out) != footer.target {
        return Err(PatchError::TargetMismatch);
    }
    Ok(out)
}
