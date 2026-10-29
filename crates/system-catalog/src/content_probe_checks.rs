//! How a [`ContentProbe`] decides a file: which extensions read bytes, and the magic
//! checks on the first [`PROBE_BYTES`](crate::PROBE_BYTES).

use crate::foreign_executable_probe::checks_elf_head;
use crate::nds_header_probe::check_nds_header;
use crate::rom_header_rules::ContentProbe;

impl ContentProbe {
    /// True when the probe needs the first [`PROBE_BYTES`](crate::PROBE_BYTES) of a
    /// file with extension `ext` (no dot, any case).
    pub fn needs_bytes(&self, ext: &str) -> bool {
        match self {
            Self::Magic { .. } | Self::MagicAny { .. } | Self::NdsHeader => true,
            Self::MagicForExtensions { .. } => self.checks_content(ext),
            // Disc probes read the image themselves; GameCube / Wii `.elf` heads are
            // checked first for a foreign CPU (`foreign_executable_probe`).
            Self::Disc(signature) => checks_elf_head(*signature, ext),
            Self::Any => false,
        }
    }

    /// True when the content of a file with extension `ext` is checked at all (not
    /// extension-only).
    pub fn checks_content(&self, ext: &str) -> bool {
        match self {
            Self::Any => false,
            Self::Magic { .. } | Self::MagicAny { .. } | Self::Disc(_) | Self::NdsHeader => true,
            Self::MagicForExtensions {
                exts, quiet_exts, ..
            } => has_ext(exts, ext) || has_ext(quiet_exts, ext),
        }
    }

    /// True when a failed check on a file with extension `ext` means the file is no
    /// ROM at all (the scanner ignores it silently, see `ClassifyError::NotRom`).
    pub fn is_quiet(&self, ext: &str) -> bool {
        matches!(self, Self::MagicForExtensions { quiet_exts, .. } if has_ext(quiet_exts, ext))
    }

    /// `Ok` when `first_bytes` (the start of a file with extension `ext`) pass, else
    /// the failure reason. A disc signature cannot be decided from bytes: the
    /// classifier hands it to a [`DiscProbe`](crate::DiscProbe) instead, so here it
    /// always fails.
    pub fn check(&self, ext: &str, first_bytes: &[u8]) -> Result<(), &'static str> {
        match *self {
            Self::Any => Ok(()),
            Self::Disc(_) => Err("disc images need a disc probe"),
            Self::NdsHeader => check_nds_header(first_bytes),
            Self::Magic {
                offset,
                bytes,
                missing,
            } => magic_at(first_bytes, &[offset], bytes, missing),
            Self::MagicAny {
                offset,
                patterns,
                missing,
            } => patterns
                .iter()
                .find_map(|bytes| magic_at(first_bytes, &[offset], bytes, missing).ok())
                .ok_or(missing),
            Self::MagicForExtensions {
                offsets,
                bytes,
                missing,
                ..
            } => {
                if self.checks_content(ext) {
                    magic_at(first_bytes, offsets, bytes, missing)
                } else {
                    Ok(())
                }
            }
        }
    }
}

fn has_ext(list: &[&str], ext: &str) -> bool {
    list.iter().any(|allowed| ext.eq_ignore_ascii_case(allowed))
}

/// `Ok` when `bytes` appear at any of `offsets` in `first_bytes`.
fn magic_at(
    first_bytes: &[u8],
    offsets: &[usize],
    bytes: &[u8],
    missing: &'static str,
) -> Result<(), &'static str> {
    let found = offsets
        .iter()
        .any(|&offset| first_bytes.get(offset..offset + bytes.len()) == Some(bytes));
    if found {
        Ok(())
    } else {
        Err(missing)
    }
}
