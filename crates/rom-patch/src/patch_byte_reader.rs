//! Cursor over patch bytes with the helpers the three formats share: fixed-width
//! big/little-endian numbers, byuu's variable-length integers (UPS/BPS) and the
//! 12-byte CRC32 footer (UPS/BPS).

use crate::patch_error::PatchError;

/// Largest patched image accepted (SNES ROMs are ≤ 12 MiB; guards allocations
/// against corrupt or hostile size fields).
pub(crate) const MAX_TARGET_SIZE: usize = 64 * 1024 * 1024;

/// Rejects target sizes above [`MAX_TARGET_SIZE`].
pub(crate) fn checked_target_size(size: usize) -> Result<usize, PatchError> {
    if size > MAX_TARGET_SIZE {
        return Err(PatchError::OutOfRange);
    }
    Ok(size)
}

pub(crate) struct PatchByteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PatchByteReader<'a> {
    /// Reader positioned after a `magic_len`-byte magic.
    pub fn new(bytes: &'a [u8], magic_len: usize) -> Self {
        Self {
            bytes,
            pos: magic_len,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8], PatchError> {
        let end = self.pos.checked_add(len).ok_or(PatchError::Truncated)?;
        let slice = self.bytes.get(self.pos..end).ok_or(PatchError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    pub fn byte(&mut self) -> Result<u8, PatchError> {
        Ok(self.take(1)?[0])
    }

    /// Big-endian unsigned integer of `len` (≤ 8) bytes.
    pub fn be(&mut self, len: usize) -> Result<usize, PatchError> {
        Ok(self
            .take(len)?
            .iter()
            .fold(0, |acc, &b| (acc << 8) | usize::from(b)))
    }

    /// byuu variable-length integer: 7 data bits per byte, high bit ends the number,
    /// each continuation adds the next power so encodings are unique.
    pub fn varint(&mut self) -> Result<usize, PatchError> {
        let (mut value, mut shift) = (0usize, 1usize);
        loop {
            let x = self.byte()?;
            value = usize::from(x & 0x7f)
                .checked_mul(shift)
                .and_then(|v| v.checked_add(value))
                .ok_or(PatchError::Truncated)?;
            if x & 0x80 != 0 {
                return Ok(value);
            }
            shift = shift.checked_mul(0x80).ok_or(PatchError::Truncated)?;
            value = value.checked_add(shift).ok_or(PatchError::Truncated)?;
        }
    }
}

/// UPS/BPS footer: CRC32 of source, target, and of the patch up to its last 4 bytes.
pub(crate) struct ChecksumFooter {
    pub source: u32,
    pub target: u32,
}

/// Validates the patch's own checksum and returns the footer.
pub(crate) fn checksum_footer(patch: &[u8]) -> Result<ChecksumFooter, PatchError> {
    let len = patch.len();
    if len < 12 {
        return Err(PatchError::Truncated);
    }
    let le =
        |at: usize| u32::from_le_bytes([patch[at], patch[at + 1], patch[at + 2], patch[at + 3]]);
    if crc32fast::hash(&patch[..len - 4]) != le(len - 4) {
        return Err(PatchError::PatchChecksum);
    }
    Ok(ChecksumFooter {
        source: le(len - 12),
        target: le(len - 8),
    })
}
