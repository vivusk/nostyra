//! What two peers must share for lockstep to hold: core build, content bytes,
//! patch, BIOS, options, region (fps), platform and session shape. Built only in the
//! runner after `retro_load_game` (phase 04 `fingerprint_source.rs`), compared in the
//! `Hello` handshake, never sent to the server. A difference refuses the joiner with
//! the exact field ("Different core version: 1.62 vs 1.63").
//!
//! Cross-platform (e.g. Windows host, macOS guest): the core binaries always differ,
//! so the core build hash is not compared; the core id + version still must match,
//! and both cores must report portable states (no platform / endian serialization
//! quirk), else "Different platform" refuses. A divergence that still happens is
//! caught by the RAM desync check and fixed with Resync.

use serde::{Deserialize, Serialize};

/// Platform tag of this build ("<os>-<arch>"), e.g. `windows-x86_64`, `macos-aarch64`.
pub const PLATFORM: &str = platform_tag();

const fn platform_tag() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "windows-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-aarch64"
    } else {
        "other"
    }
}

/// The compatibility fingerprint of one peer's loaded session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFingerprint {
    pub core_id: String,
    pub core_version: String,
    pub core_sha256: String,
    pub content_sha1: String,
    /// crc32 of the bytes the core loaded; `None` for disc content.
    pub content_crc32: Option<u32>,
    pub patch_crc32: Option<u32>,
    /// Sorted md5s of the BIOS files the core resolved; `"hle"` when it used HLE.
    pub bios_md5s: Vec<String>,
    /// Hash of the fixed + multitap core options sent at load.
    pub options_hash: u64,
    pub system: String,
    /// `av_info` fps × 1000 (mixed PAL / NTSC refused: ggrs runs one fps).
    pub fps_milli: u32,
    pub platform: String,
    /// The core's states load on other platforms (no platform / endian quirk).
    pub portable_states: bool,
    pub analog: bool,
    /// `MUST_INITIALIZE` warm-up frames before the first state (both sides equal).
    pub warmup_frames: u32,
    /// Room player cap (port devices are not core options, F21).
    pub max_players: u8,
}

/// Which field differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FingerprintField {
    CoreId,
    CoreVersion,
    CoreBuild,
    Content,
    ContentCrc,
    Patch,
    Bios,
    Options,
    System,
    Fps,
    Platform,
    Analog,
    WarmupFrames,
    MaxPlayers,
}

impl FingerprintField {
    /// Text shown to people ("Different <label>: …").
    pub fn label(self) -> &'static str {
        match self {
            Self::CoreId => "core",
            Self::CoreVersion => "core version",
            Self::CoreBuild => "core build",
            Self::Content => "game file",
            Self::ContentCrc => "game data",
            Self::Patch => "patch",
            Self::Bios => "BIOS",
            Self::Options => "core options",
            Self::System => "system",
            Self::Fps => "region / frame rate",
            Self::Platform => "platform",
            Self::Analog => "analog setting",
            Self::WarmupFrames => "start-up frames",
            Self::MaxPlayers => "room size",
        }
    }
}

/// One differing field, `ours` = the value of the fingerprint `diff` was called on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintMismatch {
    pub field: FingerprintField,
    pub ours: String,
    pub theirs: String,
}

impl FingerprintMismatch {
    /// "Different core version: 1.62 vs 1.63".
    pub fn text(&self) -> String {
        format!(
            "Different {}: {} vs {}",
            self.field.label(),
            self.ours,
            self.theirs
        )
    }
}

impl SessionFingerprint {
    /// Every field that differs, in declaration order; empty when compatible. The
    /// host calls `host.diff(guest)`, so `ours` is the host's value.
    pub fn diff(&self, other: &Self) -> Vec<FingerprintMismatch> {
        use FingerprintField as F;
        let rows = [
            (F::CoreId, self.core_id.clone(), other.core_id.clone()),
            (
                F::CoreVersion,
                self.core_version.clone(),
                other.core_version.clone(),
            ),
            (
                F::CoreBuild,
                short(&self.core_sha256),
                short(&other.core_sha256),
            ),
            (
                F::Content,
                short(&self.content_sha1),
                short(&other.content_sha1),
            ),
            (
                F::ContentCrc,
                crc(self.content_crc32),
                crc(other.content_crc32),
            ),
            (F::Patch, crc(self.patch_crc32), crc(other.patch_crc32)),
            (F::Bios, bios(&self.bios_md5s), bios(&other.bios_md5s)),
            (F::Options, hex(self.options_hash), hex(other.options_hash)),
            (F::System, self.system.clone(), other.system.clone()),
            (F::Fps, fps(self.fps_milli), fps(other.fps_milli)),
            (F::Platform, self.platform.clone(), other.platform.clone()),
            (F::Analog, onoff(self.analog), onoff(other.analog)),
            (
                F::WarmupFrames,
                self.warmup_frames.to_string(),
                other.warmup_frames.to_string(),
            ),
            (
                F::MaxPlayers,
                self.max_players.to_string(),
                other.max_players.to_string(),
            ),
        ];
        // Another OS / CPU always has another core binary: only the version counts,
        // as long as both cores' states are portable (see module doc).
        let cross_platform = self.platform != other.platform;
        let portable = self.portable_states && other.portable_states;
        // Hashes are shortened for display only: compare their full values.
        let differs = |field: F, ours: &str, theirs: &str| match field {
            F::CoreBuild => !cross_platform && self.core_sha256 != other.core_sha256,
            F::Platform => cross_platform && !portable,
            F::Content => self.content_sha1 != other.content_sha1,
            F::Bios => self.bios_md5s != other.bios_md5s,
            _ => ours != theirs,
        };
        rows.into_iter()
            .filter(|(field, ours, theirs)| differs(*field, ours, theirs))
            .map(|(field, ours, theirs)| FingerprintMismatch {
                field,
                ours,
                theirs,
            })
            .collect()
    }
}

/// First 12 hex chars of a hash (enough for a person to compare).
fn short(hash: &str) -> String {
    hash.chars().take(12).collect()
}

fn crc(value: Option<u32>) -> String {
    value.map_or_else(|| "none".to_owned(), |v| format!("{v:08x}"))
}

fn bios(md5s: &[String]) -> String {
    if md5s.is_empty() {
        return "none".to_owned();
    }
    md5s.iter().map(|m| short(m)).collect::<Vec<_>>().join(", ")
}

fn hex(value: u64) -> String {
    format!("{value:016x}")
}

fn fps(milli: u32) -> String {
    format!("{}.{:03} fps", milli / 1000, milli % 1000)
}

fn onoff(value: bool) -> String {
    if value { "on" } else { "off" }.to_owned()
}

#[cfg(test)]
#[path = "session_fingerprint_tests.rs"]
pub(crate) mod tests;
