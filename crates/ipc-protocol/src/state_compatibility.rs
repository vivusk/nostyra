//! Whether a save state (by its [`StateSidecar`]) can go into the current session,
//! beyond the core name: one rule for the runner (refuses the load before the core
//! sees the bytes) and the launcher (hides Continue, marks the slot). Dolphin accepts
//! any blob in `retro_unserialize` (M14 spike), so for such cores the host decides:
//!
//! - `strict_version` cores (`CoreSpec::strict_state_version`): a state from another
//!   core version is refused (other cores only get a warning / `core-version-changed`).
//! - Content tag: games that share one save folder across discs and revisions
//!   (GameCube / Wii) record which disc / revision a state belongs to; another tag
//!   is refused.
//!
//! Unknown facts never refuse: an empty or unknown version on either side, or no tag
//! on either side (sidecars written before M14).

use crate::save_state_files::StateSidecar;

/// Why a state cannot be loaded into this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateIncompatibility {
    /// Saved by another version of a strict-version core.
    CoreVersion,
    /// Saved from another disc or revision of the same game.
    OtherContent,
}

impl StateIncompatibility {
    /// Short reason, e.g. "made by another Dolphin version" (`core` = the registry
    /// display name of the game's core).
    pub fn reason(self, core: &str) -> String {
        match self {
            Self::CoreVersion => format!("made by another {core} version"),
            Self::OtherContent => "made from another disc or revision".to_owned(),
        }
    }
}

/// The incompatibility of `sidecar` with today's session facts, if any.
/// `core_version`: the version that would load it (`None` = unknown);
/// `content_tag`: the launched game's tag (`None` = untagged system or unreadable).
pub fn state_incompatibility(
    sidecar: &StateSidecar,
    strict_version: bool,
    core_version: Option<&str>,
    content_tag: Option<&str>,
) -> Option<StateIncompatibility> {
    let saved_version = Some(sidecar.core_version.as_str()).filter(|v| !v.is_empty());
    if let (true, Some(saved), Some(now)) = (strict_version, saved_version, core_version) {
        if saved != now {
            return Some(StateIncompatibility::CoreVersion);
        }
    }
    match (sidecar.content_tag.as_deref(), content_tag) {
        (Some(saved), Some(now)) if saved != now => Some(StateIncompatibility::OtherContent),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidecar(version: &str, tag: Option<&str>) -> StateSidecar {
        StateSidecar {
            core_name: "dolphin-emu".into(),
            core_version: version.into(),
            content_tag: tag.map(str::to_owned),
            ..StateSidecar::default()
        }
    }

    #[test]
    fn strict_cores_refuse_other_versions_lenient_ones_do_not() {
        let saved = sidecar("2609", None);
        assert_eq!(
            state_incompatibility(&saved, true, Some("2610"), None),
            Some(StateIncompatibility::CoreVersion)
        );
        assert_eq!(
            state_incompatibility(&saved, true, Some("2609"), None),
            None
        );
        assert_eq!(
            state_incompatibility(&saved, false, Some("2610"), None),
            None
        );
        // Unknown version on either side: no refusal.
        assert_eq!(state_incompatibility(&saved, true, None, None), None);
        let unknown = sidecar("", None);
        assert_eq!(
            state_incompatibility(&unknown, true, Some("2610"), None),
            None
        );
    }

    #[test]
    fn other_disc_or_revision_is_refused_untagged_sides_are_not() {
        let disc1 = sidecar("1", Some("GALE01:0:0"));
        assert_eq!(
            state_incompatibility(&disc1, true, Some("1"), Some("GALE01:0:0")),
            None
        );
        assert_eq!(
            state_incompatibility(&disc1, true, Some("1"), Some("GALE01:1:0")),
            Some(StateIncompatibility::OtherContent)
        );
        assert_eq!(
            state_incompatibility(&disc1, false, None, Some("m3u:GALE01")),
            Some(StateIncompatibility::OtherContent)
        );
        assert_eq!(state_incompatibility(&disc1, true, Some("1"), None), None);
        let old = sidecar("1", None);
        assert_eq!(
            state_incompatibility(&old, true, Some("1"), Some("GALE01:0:0")),
            None
        );
    }

    #[test]
    fn reasons_name_the_core_or_the_disc() {
        let version = StateIncompatibility::CoreVersion.reason("Dolphin");
        assert_eq!(version, "made by another Dolphin version");
        let content = StateIncompatibility::OtherContent.reason("Dolphin");
        assert_eq!(content, "made from another disc or revision");
    }
}
