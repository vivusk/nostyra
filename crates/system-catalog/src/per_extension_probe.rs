//! [`ContentProbe::PerExtension`]: one content check per extension group of a system
//! (Atari 7800 `.a78` needs `ATARI7800`, its `.bin` only a DAT hit; Lynx `.lnx` needs
//! `LYNX`, `.o` / `.bll` the BS93 magic, `.lyx` nothing). Checks only read the first
//! [`PROBE_BYTES`](crate::PROBE_BYTES); a [`ExtCheck::DatHash`] group is never
//! decided by the classifier (see [`ContentProbe::is_dat_gated`]).

use crate::rom_header_rules::ContentProbe;
use crate::system_specs::SystemSpec;

/// The check for the extensions in `exts` (lower case, no dot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionCheck {
    pub exts: &'static [&'static str],
    pub check: ExtCheck,
}

/// What a file of one extension group must show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtCheck {
    /// No check: the extension decides.
    Any,
    /// `bytes` at `offset`; `missing` is the reason on a miss. `quiet`: a miss means
    /// the file is no ROM at all (a C object `.o`), ignored without a problem row.
    Magic {
        offset: usize,
        bytes: &'static [u8],
        missing: &'static str,
        quiet: bool,
    },
    /// Listed only when the file's hashes are in the system's DAT (a headerless
    /// `.bin` shared with Mega Drive and disc images): the classifier skips this
    /// candidate; the scanner looks the file up later (`dat_identified_candidates`).
    DatHash,
}

impl ExtensionCheck {
    /// End of the magic bytes (for the registry's probe-size test), if any.
    pub fn magic_end(&self) -> Option<usize> {
        match self.check {
            ExtCheck::Magic { offset, bytes, .. } => Some(offset + bytes.len()),
            ExtCheck::Any | ExtCheck::DatHash => None,
        }
    }
}

/// The check of the group holding `ext` (any case), if one does.
pub(crate) fn check_for(checks: &[ExtensionCheck], ext: &str) -> Option<ExtCheck> {
    checks
        .iter()
        .find(|group| group.exts.iter().any(|e| ext.eq_ignore_ascii_case(e)))
        .map(|group| group.check)
}

impl ContentProbe {
    /// True when a file with extension `ext` is listed only through a DAT hit
    /// ([`ExtCheck::DatHash`]): the classifier neither accepts nor rejects it.
    pub fn is_dat_gated(&self, ext: &str) -> bool {
        matches!(self, Self::PerExtension(checks) if check_for(checks, ext) == Some(ExtCheck::DatHash))
    }
}

/// Systems among `systems` (in their order) that list a file with extension `ext`
/// and whole-file length `file_len` only through a DAT hit: the candidates the
/// scanner looks the file up for after classification found no system.
pub fn dat_identified_candidates(
    systems: &[&'static SystemSpec],
    ext: &str,
    file_len: u64,
) -> Vec<&'static SystemSpec> {
    systems
        .iter()
        .copied()
        .filter(|spec| spec.has_extension(ext) && spec.accepts_size(file_len))
        .filter(|spec| spec.probe.is_dat_gated(ext))
        .collect()
}
