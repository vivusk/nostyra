//! Why the classifier found no system for a file, and which of two candidate
//! rejects is the one reported.

/// Why a file has no system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyError {
    /// No system uses this extension (the scanner ignores such files silently).
    NoCandidate,
    /// Every candidate system rejects the file size.
    OutOfSize,
    /// The size fits but the content probe failed ("missing iNES header").
    ProbeFailed(&'static str),
    /// The content shows the file is no ROM of this extension at all (a Markdown
    /// `README.md` among `.md` Mega Drive dumps): the scanner ignores it silently,
    /// the runner shows the reason.
    NotRom(&'static str),
    /// A disc probe rejected the image ("track file 'X' not found", "not a
    /// PlayStation disc").
    DiscRejected(String),
    /// The file could not be read.
    Io(String),
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCandidate => f.write_str("no supported system uses this file extension"),
            Self::OutOfSize => f.write_str("file size is outside the system limits"),
            Self::ProbeFailed(reason) | Self::NotRom(reason) => f.write_str(reason),
            Self::DiscRejected(reason) => f.write_str(reason),
            Self::Io(err) => write!(f, "cannot read file: {err}"),
        }
    }
}

impl std::error::Error for ClassifyError {}

impl ClassifyError {
    /// True when `self` (an earlier candidate's reject) is kept over `later`: a
    /// specific disc reason (a UMD Video disc, a damaged container, a missing track)
    /// is never replaced by a later candidate's plain "not a … disc" (the GameCube /
    /// Wii probes come last on `.iso` and only know "no GC / Wii magic here").
    pub(crate) fn outranks(&self, later: &ClassifyError) -> bool {
        match (self, later) {
            (Self::DiscRejected(kept), Self::DiscRejected(new)) => {
                !is_generic_reject(kept) && is_generic_reject(new)
            }
            _ => false,
        }
    }
}

/// A disc probe's "this is not my disc" text: the `not a <systems> disc` /
/// `not a <systems> playlist` family ("not a PlayStation disc", "not a PlayStation,
/// PSP, GameCube or Wii disc", "not a PlayStation, Dreamcast or GameCube playlist")
/// and PSP's "not a PSP UMD image". Reasons about the file itself stay specific even
/// when they start the same way ("not a valid GDI sheet", "not a valid DiscJuggler
/// image", "not a single disc image", "not a PSP eboot (…)").
fn is_generic_reject(reason: &str) -> bool {
    let Some(what) = reason.strip_prefix("not a ") else {
        return false;
    };
    !what.starts_with("valid ")
        && (what.ends_with(" disc") || what.ends_with(" playlist") || what == "PSP UMD image")
}

#[cfg(test)]
mod tests {
    use super::ClassifyError::{DiscRejected, NotRom, OutOfSize};

    #[test]
    fn a_specific_disc_reason_outranks_a_later_generic_one_only() {
        let specific = DiscRejected("UMD Video/Audio disc".into());
        let generic = DiscRejected("not a PlayStation, PSP, GameCube or Wii disc".into());
        assert!(specific.outranks(&generic));
        assert!(
            !generic.outranks(&specific),
            "a reason replaces a generic text"
        );
        assert!(!generic.outranks(&DiscRejected("not a PSP UMD image".into())));
        let other = DiscRejected("track file 'x.bin' not found".into());
        assert!(
            !specific.outranks(&other),
            "later specific reasons still win"
        );
        assert!(!OutOfSize.outranks(&generic) && !specific.outranks(&NotRom("x")));
    }

    #[test]
    fn file_reasons_starting_like_a_generic_text_stay_specific() {
        let playlist = DiscRejected("not a PlayStation, Dreamcast or GameCube playlist".into());
        let disc = DiscRejected("not a GameCube or Wii disc".into());
        for reason in [
            "not a valid GDI sheet",
            "not a valid DiscJuggler image",
            "not a single disc image",
            "not a PSP eboot (PlayStation classic or no game data)",
        ] {
            let kept = DiscRejected(reason.into());
            assert!(kept.outranks(&playlist) && kept.outranks(&disc), "{reason}");
        }
        for generic in [
            "not a Dreamcast disc",
            "not a PSP UMD image",
            "not a Wii disc",
        ] {
            assert!(!DiscRejected(generic.into()).outranks(&disc), "{generic}");
        }
    }
}
