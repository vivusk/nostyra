//! IPS: `PATCH`, records of (3-byte offset, 2-byte size, data) — size 0 means an RLE
//! record (2-byte count, 1 fill byte) — then `EOF` and an optional 3-byte truncation
//! size. No checksums. Records may write past the end (the image grows, zero-filled).

use crate::patch_byte_reader::PatchByteReader;
use crate::patch_error::PatchError;

/// The `EOF` marker read as a record offset.
const EOF_MARKER: usize = 0x45_4F_46;

pub(crate) fn apply(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError> {
    let mut reader = PatchByteReader::new(patch, 5);
    let mut out = rom.to_vec();
    loop {
        let offset = reader.be(3)?;
        if offset == EOF_MARKER {
            break;
        }
        let size = reader.be(2)?;
        let (len, fill) = if size == 0 {
            (reader.be(2)?, Some(reader.byte()?))
        } else {
            (size, None)
        };
        let end = offset + len; // ≤ 16 MiB + 64 KiB: 3-byte offset, 2-byte length
        if out.len() < end {
            out.resize(end, 0);
        }
        match fill {
            Some(value) => out[offset..end].fill(value),
            None => out[offset..end].copy_from_slice(reader.take(len)?),
        }
    }
    // Optional truncation extension (3 bytes after EOF).
    if patch.len() - reader.pos() == 3 {
        out.truncate(reader.be(3)?);
    }
    Ok(out)
}
