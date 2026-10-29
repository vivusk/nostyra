//! Header-aware CRC32 + SHA1 of a ROM in one pass over the bytes.
//!
//! What is hashed follows the system's `HeaderRule`: SNES skips a copier header
//! (No-Intro hashes headerless SNES dumps); NES hashes the whole file
//! (libretro-database lists headered `.nes` dumps) with a headerless fallback for
//! re-headered copies; Mega Drive SMD files hash as their de-interleaved raw image;
//! N64 `.v64` / `.n64` dumps hash as their big-endian `.z64` image. Large
//! whole-file cartridges (DS, up to 512 MiB) are hashed streaming, never held in RAM.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha1::{Digest, Sha1};
use system_catalog::{
    ines_header_len, normalise_in_place, rom_image_for_hash, HeaderRule, SystemSpec,
};

/// Hashes of the bytes a DAT lists (upper-case hex).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomHashes {
    pub crc32: String,
    pub sha1: String,
    /// Hashed size (what DAT `size` refers to).
    pub size: u64,
}

/// Whole-file cartridges larger than this are hashed streaming by [`hash_rom_file`]
/// (4 scan threads × a 512 MiB DS dump would otherwise hold 2 GiB).
pub const STREAM_HASH_THRESHOLD: u64 = 32 * 1024 * 1024;

/// Reads and hashes a cartridge ROM file (see [`read_rom_file`] and
/// [`hash_rom_bytes`]) and returns the bytes too. The buffer is owned, so an N64
/// `.v64` / `.n64` dump is turned into its `.z64` image in place (no second
/// 64 MiB copy); the returned bytes are that image. A `HeaderRule::None` file over
/// [`STREAM_HASH_THRESHOLD`] is hashed streaming and comes back with **no bytes**
/// (an empty `Vec`; whole-file rules need no bytes after hashing).
pub fn hash_rom_file(path: &Path, system: &SystemSpec) -> std::io::Result<(RomHashes, Vec<u8>)> {
    hash_rom_file_with(path, system, STREAM_HASH_THRESHOLD)
}

/// [`hash_rom_file`] with the streaming threshold given (tests use a small one).
pub(crate) fn hash_rom_file_with(
    path: &Path,
    system: &SystemSpec,
    stream_above: u64,
) -> std::io::Result<(RomHashes, Vec<u8>)> {
    if system.header_rule == HeaderRule::None && std::fs::metadata(path)?.len() > stream_above {
        let hashes = hash_reader(File::open(path)?.take(system.max_bytes.saturating_add(1)))?;
        if hashes.size > system.max_bytes {
            return Err(too_large());
        }
        return Ok((hashes, Vec::new()));
    }
    let mut bytes = read_rom_file(path, system)?;
    if system.header_rule == HeaderRule::N64ByteOrder {
        normalise_in_place(&mut bytes);
    }
    let hashes = hash_rom_bytes(&bytes, system);
    Ok((hashes, bytes))
}

/// Primary hashes of a ROM: the image `rom_image_for_hash` gives for the system's
/// header rule (copier header skipped, SMD de-interleaved, N64 byte order turned
/// into `.z64`; whole file for NES).
pub fn hash_rom_bytes(bytes: &[u8], system: &SystemSpec) -> RomHashes {
    hash_image(&rom_image_for_hash(system.header_rule, bytes))
}

/// NES fallback: hashes of the bytes after the iNES header (+ trainer), matching the
/// DAT's headerless `.unh` entries. `None` for other header rules or a file with no
/// data after the header. Called only when the primary hashes miss the DAT.
pub fn hash_headerless(bytes: &[u8], system: &SystemSpec) -> Option<RomHashes> {
    if system.header_rule != HeaderRule::INes {
        return None;
    }
    let skip = ines_header_len(bytes);
    (skip > 0 && skip < bytes.len()).then(|| hash_image(&bytes[skip..]))
}

fn hash_image(image: &[u8]) -> RomHashes {
    let mut crc = crc32fast::Hasher::new();
    crc.update(image);
    let mut sha1 = Sha1::new();
    sha1.update(image);
    RomHashes {
        crc32: format!("{:08X}", crc.finalize()),
        sha1: upper_hex(&sha1.finalize()),
        size: image.len() as u64,
    }
}

/// Reads the whole ROM for hashing. Reading stops one byte past `system.max_bytes`,
/// so a file that grew after the scanner's size check cannot exhaust memory.
pub fn read_rom_file(path: &Path, system: &SystemSpec) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(system.max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > system.max_bytes {
        return Err(too_large());
    }
    Ok(bytes)
}

fn too_large() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "file is larger than the platform size limit",
    )
}

/// CRC32 + SHA1 of a whole file read in 1 MiB chunks: disc tracks are hundreds of
/// MB and are never held in memory (cartridges use [`read_rom_file`] up to
/// [`STREAM_HASH_THRESHOLD`]).
pub fn hash_file_streaming(path: &Path) -> std::io::Result<RomHashes> {
    hash_reader(File::open(path)?)
}

/// CRC32 + SHA1 of an in-memory key (a GameCube / Wii canonical game-ID text), with
/// `size` reported as the hashed size (the file's, for DAT size tie-breaks).
pub fn hash_bytes(bytes: &[u8], size: u64) -> RomHashes {
    RomHashes {
        size,
        ..hash_image(bytes)
    }
}

fn hash_reader(mut file: impl Read) -> std::io::Result<RomHashes> {
    let mut buf = vec![0u8; 1024 * 1024];
    let mut crc = crc32fast::Hasher::new();
    let mut sha1 = Sha1::new();
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        crc.update(&buf[..n]);
        sha1.update(&buf[..n]);
        size += n as u64;
    }
    Ok(RomHashes {
        crc32: format!("{:08X}", crc.finalize()),
        sha1: upper_hex(&sha1.finalize()),
        size,
    })
}

fn upper_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
#[path = "rom_file_hasher_tests.rs"]
mod tests;
