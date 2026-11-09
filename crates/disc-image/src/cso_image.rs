//! Compressed ISO (`.cso`, CISO v0/v1, and `.zso`, the LZ4 variant with the same
//! layout): a 24-byte header, an index of `blocks + 1` u32 entries (bit 31 = stored
//! plain, low bits = file offset >> align), then the blocks, each raw-deflated (CISO)
//! / an LZ4 block (ZISO) or plain. Read one 2048-byte sector at a time with one
//! decompressed block cached; the index is read entry by entry (a UMD's index is a
//! few MB, a PS2 DVD's tens of MB).

use content_file_source::ContentFile;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use flate2::read::DeflateDecoder;

use crate::disc_sector_reader::{DiscReader, SectorData, SECTOR_DATA_BYTES};
use crate::DiscError;

const HEADER_BYTES: u64 = 0x18;
const PLAIN_BIT: u32 = 0x8000_0000;
/// Largest block size accepted (real images use 2 KiB; some tools 16 KiB).
const MAX_BLOCK_BYTES: u32 = 1024 * 1024;
/// Most alignment padding bytes tried after an LZ4 block (real ZSOs align to ≤ 4).
const MAX_LZ4_PADDING: usize = 64;

pub struct CsoReader {
    file: BufReader<ContentFile>,
    block_bytes: u64,
    align: u32,
    total_bytes: u64,
    /// `ZISO` magic: blocks are LZ4 blocks instead of raw deflate streams.
    lz4: bool,
    block: Vec<u8>,
    cached: Option<u64>,
}

impl CsoReader {
    pub fn open(path: &Path) -> Result<Self, DiscError> {
        let mut file = BufReader::new(content_file_source::open(path)?);
        let mut header = [0u8; HEADER_BYTES as usize];
        file.read_exact(&mut header)
            .map_err(|_| DiscError::Format("file too short for a CSO header"))?;
        let lz4 = match &header[..4] {
            b"CISO" => false,
            b"ZISO" => true,
            _ => return Err(DiscError::Format("not a CSO / ZSO file")),
        };
        let total_bytes = u64::from_le_bytes(header[8..16].try_into().unwrap_or_default());
        let block_bytes = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
        if header[20] > 1 {
            return Err(DiscError::Format("CSO version 2 is not supported"));
        }
        if !block_bytes.is_power_of_two()
            || !(SECTOR_DATA_BYTES as u32..=MAX_BLOCK_BYTES).contains(&block_bytes)
        {
            return Err(DiscError::Format("CSO block size is not supported"));
        }
        Ok(Self {
            file,
            block_bytes: u64::from(block_bytes),
            align: u32::from(header[21]).min(31),
            total_bytes,
            lz4,
            block: Vec::new(),
            cached: None,
        })
    }

    /// Whether the image is a ZSO (`ZISO` magic, LZ4 blocks).
    pub fn is_lz4(&self) -> bool {
        self.lz4
    }

    /// Decompresses block `index` into `self.block`.
    fn load_block(&mut self, index: u64) -> Result<(), DiscError> {
        const CORRUPT: DiscError = DiscError::Format("CSO index is corrupt");
        self.cached = None;
        self.file.seek(SeekFrom::Start(HEADER_BYTES + index * 4))?;
        let mut pair = [0u8; 8];
        self.file.read_exact(&mut pair).map_err(|_| CORRUPT)?;
        let entry = u32::from_le_bytes([pair[0], pair[1], pair[2], pair[3]]);
        let next = u32::from_le_bytes([pair[4], pair[5], pair[6], pair[7]]);
        let offset = |e: u32| u64::from(e & !PLAIN_BIT) << self.align;
        let (start, end) = (offset(entry), offset(next));
        let stored = end.checked_sub(start).ok_or(CORRUPT)?;
        // A compressed block may be slightly larger than the plain one, never 2×.
        if stored == 0 || stored > self.block_bytes * 2 {
            return Err(CORRUPT);
        }
        let want = (self.total_bytes - index * self.block_bytes).min(self.block_bytes);
        self.file.seek(SeekFrom::Start(start))?;
        let mut raw = (&mut self.file).take(stored);
        self.block.clear();
        if entry & PLAIN_BIT != 0 {
            raw.take(want)
                .read_to_end(&mut self.block)
                .map_err(|_| DiscError::Format("CSO block is unreadable"))?;
        } else if self.lz4 {
            let mut packed = Vec::with_capacity(stored as usize);
            raw.read_to_end(&mut packed)
                .map_err(|_| DiscError::Format("ZSO block is unreadable"))?;
            self.block.resize(want as usize, 0);
            let padding = ((1usize << self.align) - 1).min(MAX_LZ4_PADDING);
            if decompress_lz4(&packed, &mut self.block, padding).is_none() {
                return Err(DiscError::Format("ZSO block does not decompress"));
            }
        } else {
            DeflateDecoder::new(raw)
                .take(want)
                .read_to_end(&mut self.block)
                .map_err(|_| DiscError::Format("CSO block does not decompress"))?;
        }
        if (self.block.len() as u64) < want {
            return Err(DiscError::Format("CSO block is truncated"));
        }
        self.cached = Some(index);
        Ok(())
    }
}

/// Decodes one LZ4 block into the whole of `out`. The index only says where the
/// next block starts, so `packed` may end in up to `padding` alignment bytes; an
/// LZ4 block has no end marker and the decoder reads padding as another sequence
/// (the cores use `LZ4_decompress_safe_partial`, which stops when `out` is full),
/// so trailing bytes are dropped one at a time until the block fills `out` exactly.
fn decompress_lz4(packed: &[u8], out: &mut [u8], padding: usize) -> Option<usize> {
    (0..=padding.min(packed.len())).find_map(|cut| {
        let len = lz4_flex::block::decompress_into(&packed[..packed.len() - cut], out).ok()?;
        (len == out.len()).then_some(len)
    })
}

impl DiscReader for CsoReader {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError> {
        let byte = u64::from(lba) * SECTOR_DATA_BYTES as u64;
        if byte + SECTOR_DATA_BYTES as u64 > self.total_bytes {
            return Err(DiscError::SectorOutOfRange(lba));
        }
        let index = byte / self.block_bytes;
        if self.cached != Some(index) {
            self.load_block(index)?;
        }
        let at = (byte % self.block_bytes) as usize;
        let mut data = [0u8; SECTOR_DATA_BYTES];
        let src = self
            .block
            .get(at..at + SECTOR_DATA_BYTES)
            .ok_or(DiscError::SectorOutOfRange(lba))?;
        data.copy_from_slice(src);
        Ok(data)
    }

    fn sector_count(&self) -> u64 {
        self.total_bytes / SECTOR_DATA_BYTES as u64
    }
}

#[cfg(test)]
#[path = "cso_image_tests.rs"]
mod tests;
