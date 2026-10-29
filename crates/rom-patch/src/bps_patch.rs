//! BPS: `BPS1`, source/target/metadata sizes (varints), metadata, then actions until
//! the CRC32 footer. Each action is `varint (length-1) << 2 | command`:
//! 0 SourceRead (copy source at the output position), 1 TargetRead (literal bytes),
//! 2 SourceCopy / 3 TargetCopy (signed relative offset varint, then copy; TargetCopy
//! may overlap its own output, so it copies byte by byte). Checksums are verified.

use crate::patch_byte_reader::{checked_target_size, checksum_footer, PatchByteReader};
use crate::patch_error::PatchError;

pub(crate) fn apply(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError> {
    let footer = checksum_footer(patch)?;
    let mut reader = PatchByteReader::new(patch, 4);
    let source_size = reader.varint()?;
    let target_size = checked_target_size(reader.varint()?)?;
    let metadata_size = reader.varint()?;
    reader.take(metadata_size)?;
    if rom.len() != source_size || crc32fast::hash(rom) != footer.source {
        return Err(PatchError::SourceMismatch);
    }
    let mut out: Vec<u8> = Vec::with_capacity(target_size);
    let (mut source_rel, mut target_rel) = (0usize, 0usize);
    let actions_end = patch.len() - 12;
    while reader.pos() < actions_end {
        let data = reader.varint()?;
        let len = (data >> 2) + 1;
        match data & 3 {
            0 => {
                let at = out.len();
                out.extend_from_slice(rom.get(at..at + len).ok_or(PatchError::OutOfRange)?);
            }
            1 => out.extend_from_slice(reader.take(len)?),
            2 => {
                source_rel = relative(source_rel, reader.varint()?)?;
                let chunk = rom
                    .get(source_rel..source_rel + len)
                    .ok_or(PatchError::OutOfRange)?;
                out.extend_from_slice(chunk);
                source_rel += len;
            }
            _ => {
                target_rel = relative(target_rel, reader.varint()?)?;
                for _ in 0..len {
                    let byte = *out.get(target_rel).ok_or(PatchError::OutOfRange)?;
                    out.push(byte);
                    target_rel += 1;
                }
            }
        }
        if out.len() > target_size {
            return Err(PatchError::OutOfRange);
        }
    }
    if out.len() != target_size || crc32fast::hash(&out) != footer.target {
        return Err(PatchError::TargetMismatch);
    }
    Ok(out)
}

/// Applies a BPS signed offset (low bit = sign, rest = magnitude).
fn relative(base: usize, encoded: usize) -> Result<usize, PatchError> {
    let magnitude = encoded >> 1;
    let moved = if encoded & 1 == 1 {
        base.checked_sub(magnitude)
    } else {
        base.checked_add(magnitude)
    };
    moved.ok_or(PatchError::OutOfRange)
}
