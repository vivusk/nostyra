//! The runner's half of the compatibility handshake (F4): every
//! `SessionFingerprint` field computed after `retro_load_game`, before the window
//! opens. Core build (sha256 of the DLL), content bytes the core actually loaded
//! (the patched bytes when soft-patched, F27), BIOS files resolved in the system
//! folder, the option map sent to the core (fixed + multitap), timing and shape.

use std::path::Path;

use md5::Md5;
use netplay_session::session_fingerprint::{SessionFingerprint, PLATFORM};
use sha2::{Digest, Sha256};
use system_catalog::{CoreSpec, Media, SystemSpec};

use crate::rom_soft_patching::SoftPatchedRom;

/// Everything the fingerprint is built from (one struct: the caller is the loop).
pub(crate) struct FingerprintInputs<'a> {
    pub core: &'a CoreSpec,
    pub system: &'a SystemSpec,
    pub core_path: &'a Path,
    pub core_version: &'a str,
    pub content_sha1: &'a str,
    /// The file the core loaded (cartridges: read once unless patched).
    pub content_rom: &'a Path,
    pub patched: &'a SoftPatchedRom,
    pub system_dir: &'a Path,
    /// The option map handed to the core at load.
    pub core_options: &'a [(String, String)],
    pub fps: f64,
    /// The core reports no platform / endian serialization quirk.
    pub portable_states: bool,
    pub warmup_frames: u32,
    pub max_players: u8,
}

/// Builds the fingerprint; `Err` only when the core file cannot be read.
pub(crate) fn build_fingerprint(
    input: &FingerprintInputs<'_>,
) -> std::io::Result<SessionFingerprint> {
    let core_sha256 = hex(&Sha256::digest(std::fs::read(input.core_path)?));
    Ok(SessionFingerprint {
        core_id: input.core.id.to_owned(),
        core_version: input.core_version.to_owned(),
        core_sha256,
        content_sha1: input.content_sha1.to_owned(),
        content_crc32: content_crc32(input.system.media, input.patched, input.content_rom),
        patch_crc32: input.patched.data.as_ref().and(input.patched.patch_crc32),
        bios_md5s: bios_md5s(input.core, input.system_dir, input.core_options),
        options_hash: options_hash(input.core_options),
        system: input.system.id.to_owned(),
        fps_milli: (input.fps * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32,
        platform: PLATFORM.to_owned(),
        portable_states: input.portable_states,
        analog: input.system.analog_sticks != system_catalog::AnalogSticks::None,
        warmup_frames: input.warmup_frames,
        max_players: input.max_players,
    })
}

/// crc32 of the bytes the core loaded: patched bytes, else the file; discs `None`.
pub(crate) fn content_crc32(media: Media, patched: &SoftPatchedRom, rom: &Path) -> Option<u32> {
    match (media, &patched.data) {
        (Media::Disc, _) => None,
        (Media::Cartridge, Some(bytes)) => Some(crc32fast::hash(bytes)),
        (Media::Cartridge, None) => std::fs::read(rom).ok().map(|b| crc32fast::hash(&b)),
    }
}

/// sha256 over the sorted `key=value` lines, first 8 bytes (order-independent).
pub(crate) fn options_hash(options: &[(String, String)]) -> u64 {
    let mut sorted: Vec<&(String, String)> = options.iter().collect();
    sorted.sort();
    let mut hasher = Sha256::new();
    for (key, value) in sorted {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(first)
}

/// Sorted md5s of the core's known firmware files present in the system folder;
/// `["hle"]` when a `*_bios` option forces HLE or the core lists firmware but none
/// is present (PCSX `bios=auto` without a file).
pub(crate) fn bios_md5s(
    core: &CoreSpec,
    system_dir: &Path,
    options: &[(String, String)],
) -> Vec<String> {
    let hle = || vec!["hle".to_owned()];
    let forced_hle = options
        .iter()
        .any(|(key, value)| key.ends_with("_bios") && value.eq_ignore_ascii_case("hle"));
    if forced_hle {
        return hle();
    }
    let mut md5s: Vec<String> = core
        .firmware
        .iter()
        .filter_map(|file| std::fs::read(system_dir.join(file.subdir).join(file.name)).ok())
        .map(|bytes| hex(&Md5::digest(&bytes)))
        .collect();
    md5s.sort();
    md5s.dedup();
    if md5s.is_empty() && !core.firmware.is_empty() {
        return hle();
    }
    md5s
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "fingerprint_source_tests.rs"]
mod tests;
