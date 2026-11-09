//! A > 4 GiB disc image served from memory with zero bytes on disk: [`VirtualDisc`]
//! is a `ContentFileSource` (the web edition's door, `content_file_source::with_source`,
//! thread-local) serving one zero-filled image of `(2_097_152 + 16)` sectors from a
//! position counter, with a 16-byte marker in the user data of sector [`MARKER_LBA`]
//! (byte offset past 4 GiB: PlayStation 2 dual-layer DVDs reach 8.5 GB). Installed
//! per test, `FileTrackReader::open_auto`, `DiscTrack::open` and the achievements
//! crate's rcheevos cdreader read it like a file (`disc_reader_lba_tests`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use content_file_source::{ContentFile, ContentFileSource};

use crate::disc_sector_reader::{RAW_SECTOR_BYTES, SECTOR_DATA_BYTES};
use crate::{BIN_EXT, ISO_EXT};

/// The sector that carries the marker: `2_097_157 × 2048` = 4 GiB + 10 KiB.
pub const MARKER_LBA: u32 = 2_097_157;
/// What the marker sector's user data starts with (the rest of the sector is zero).
pub const MARKER: [u8; 16] = *b"NOSTYRA>4GIB-LBA";
/// Sectors of the virtual image (the marker sector plus a few more).
pub const SECTORS: u64 = 2_097_152 + 16;
const SYNC: [u8; 12] = [
    0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0,
];

/// One virtual disc image: a 2048-byte `.iso` or a raw 2352-byte MODE1 `.bin`.
pub struct VirtualDisc {
    path: PathBuf,
    sector_bytes: u64,
    data_offset: u64,
}

impl VirtualDisc {
    /// A plain 2048-byte-sector `.iso` at [`Self::path`].
    pub fn iso() -> Self {
        Self {
            path: Path::new("/virtual/ps2-dvd9").with_extension(ISO_EXT),
            sector_bytes: SECTOR_DATA_BYTES as u64,
            data_offset: 0,
        }
    }

    /// A raw MODE1/2352 `.bin` (sync pattern + header per sector, user data at 16).
    pub fn raw_bin() -> Self {
        Self {
            path: Path::new("/virtual/ps2-cd").with_extension(BIN_EXT),
            sector_bytes: RAW_SECTOR_BYTES,
            data_offset: 16,
        }
    }

    /// The one path this source serves.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Byte offset of the marker inside the image.
    pub fn marker_offset(&self) -> u64 {
        u64::from(MARKER_LBA) * self.sector_bytes + self.data_offset
    }

    fn len_bytes(&self) -> u64 {
        SECTORS * self.sector_bytes
    }
}

impl ContentFileSource for VirtualDisc {
    fn open(&self, path: &Path) -> io::Result<ContentFile> {
        if path != self.path {
            return Err(io::Error::new(io::ErrorKind::NotFound, "virtual disc"));
        }
        Ok(Box::new(ZeroStream {
            pos: 0,
            len: self.len_bytes(),
            sector_bytes: self.sector_bytes,
            data_offset: self.data_offset,
        }))
    }

    fn len(&self, path: &Path) -> io::Result<u64> {
        self.is_file(path)
            .then(|| self.len_bytes())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "virtual disc"))
    }

    fn is_file(&self, path: &Path) -> bool {
        path == self.path
    }
}

/// Bytes of the virtual image computed on demand from the position.
struct ZeroStream {
    pos: u64,
    len: u64,
    sector_bytes: u64,
    data_offset: u64,
}

impl ZeroStream {
    fn byte_at(&self, pos: u64) -> u8 {
        let (sector, within) = (pos / self.sector_bytes, pos % self.sector_bytes);
        if self.data_offset == 16 && within < 16 {
            // Raw sector header: sync, (zero) MSF, mode 1.
            return match within {
                0..=11 => SYNC[within as usize],
                15 => 1,
                _ => 0,
            };
        }
        let data_index = within.wrapping_sub(self.data_offset);
        if u64::from(MARKER_LBA) == sector && data_index < MARKER.len() as u64 {
            return MARKER[data_index as usize];
        }
        0
    }
}

impl Read for ZeroStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.len.saturating_sub(self.pos);
        let n = (buf.len() as u64).min(left) as usize;
        for (i, byte) in buf[..n].iter_mut().enumerate() {
            *byte = self.byte_at(self.pos + i as u64);
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for ZeroStream {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let (base, delta) = match to {
            SeekFrom::Start(at) => (at, 0),
            SeekFrom::End(delta) => (self.len, delta),
            SeekFrom::Current(delta) => (self.pos, delta),
        };
        self.pos = base
            .checked_add_signed(delta)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}

#[cfg(test)]
#[path = "disc_reader_lba_tests.rs"]
mod tests;
