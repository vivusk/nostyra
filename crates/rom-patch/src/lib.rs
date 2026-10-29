//! Soft-patching (as RetroArch does it): patch files next to the ROM with the same
//! name (`Game.sfc` + `Game.bps`, `Game.ips`, `Game.ips1`, …) are applied to the ROM
//! bytes in memory, in chain order, when the game starts; the ROM file itself is never
//! modified. Formats: IPS, UPS, BPS (detected by their magic bytes). Pure Rust, no I/O
//! except [`find_patches_for_rom`].

#![deny(unsafe_code)]

mod bps_patch;
mod ips_patch;
mod patch_byte_reader;
mod patch_error;
mod patch_file_lookup;
mod ups_patch;

pub use patch_error::PatchError;
pub use patch_file_lookup::{find_patches_for_rom, PATCH_EXTENSIONS};

/// Size of the copier header some SNES dumps carry (file size = 1024·n + 512).
const COPIER_HEADER: usize = 512;

/// Patch format, from the file's magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchFormat {
    Ips,
    Ups,
    Bps,
}

impl PatchFormat {
    /// Detects the format from the first bytes of a patch file.
    pub fn detect(patch: &[u8]) -> Option<Self> {
        if patch.starts_with(b"PATCH") {
            Some(Self::Ips)
        } else if patch.starts_with(b"UPS1") {
            Some(Self::Ups)
        } else if patch.starts_with(b"BPS1") {
            Some(Self::Bps)
        } else {
            None
        }
    }

    /// Short upper-case name for UI ("IPS").
    pub fn label(self) -> &'static str {
        match self {
            Self::Ips => "IPS",
            Self::Ups => "UPS",
            Self::Bps => "BPS",
        }
    }
}

/// Applies `patch` to `rom` and returns the patched image.
///
/// UPS/BPS check the source checksum; when it does not match and the ROM carries a
/// 512-byte copier header, the patch is retried on the headerless image (most patches
/// are made for headerless dumps; cores accept either).
pub fn apply_patch(rom: &[u8], patch: &[u8]) -> Result<(PatchFormat, Vec<u8>), PatchError> {
    let format = PatchFormat::detect(patch).ok_or(PatchError::UnknownFormat)?;
    let apply = |source: &[u8]| match format {
        PatchFormat::Ips => ips_patch::apply(source, patch),
        PatchFormat::Ups => ups_patch::apply(source, patch),
        PatchFormat::Bps => bps_patch::apply(source, patch),
    };
    match apply(rom) {
        Err(PatchError::SourceMismatch) if rom.len() % 1024 == COPIER_HEADER => {
            apply(&rom[COPIER_HEADER..]).map(|out| (format, out))
        }
        result => result.map(|out| (format, out)),
    }
}

/// Applies a patch chain in order, each patch to the previous output. Returns the
/// formats applied and the final image; on failure, the index of the patch that
/// failed (a partly patched image is never returned).
pub fn apply_patch_chain<P: AsRef<[u8]>>(
    rom: &[u8],
    patches: &[P],
) -> Result<(Vec<PatchFormat>, Vec<u8>), (usize, PatchError)> {
    let mut formats = Vec::with_capacity(patches.len());
    let mut image = rom.to_vec();
    for (index, patch) in patches.iter().enumerate() {
        let (format, patched) = apply_patch(&image, patch.as_ref()).map_err(|err| (index, err))?;
        formats.push(format);
        image = patched;
    }
    Ok((formats, image))
}

/// CRC32 of a patch chain: the CRC of all patch files concatenated in chain order
/// (one patch = that file's CRC, so sidecars written before chains still match).
/// `None` for an empty chain.
pub fn patch_chain_crc32<P: AsRef<[u8]>>(patches: &[P]) -> Option<u32> {
    if patches.is_empty() {
        return None;
    }
    let mut hasher = crc32fast::Hasher::new();
    for patch in patches {
        hasher.update(patch.as_ref());
    }
    Some(hasher.finalize())
}
