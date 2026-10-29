//! Which cores' save states may drive run-ahead and rewind. Both restore a state every
//! frame or two, so they need states that capture the exact frame: refused when the
//! core reports incomplete states (`SERIALIZATION_QUIRK_INCOMPLETE`) or the registry
//! marks its states `Serialized` (serialize advances the emulation, e.g. N64).

use system_catalog::SaveStateQuality;

/// Pause-menu reason: the core reports incomplete save states.
pub(crate) const INCOMPLETE_STATES: &str = "Core save states incomplete";
/// Pause-menu reason: serializing advances the emulation (registry `Serialized`).
pub(crate) const STATES_NOT_FRAME_EXACT: &str = "Core save states are not frame-exact";

/// Why run-ahead and rewind cannot use this session's states, if they cannot.
/// `incomplete`: the core's serialization quirk.
pub(crate) fn state_refusal(incomplete: bool, quality: SaveStateQuality) -> Option<&'static str> {
    match quality {
        SaveStateQuality::Serialized => Some(STATES_NOT_FRAME_EXACT),
        SaveStateQuality::Deterministic if incomplete => Some(INCOMPLETE_STATES),
        SaveStateQuality::Deterministic => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_or_incomplete_states_are_refused() {
        use SaveStateQuality::{Deterministic, Serialized};
        assert_eq!(state_refusal(false, Deterministic), None);
        assert_eq!(state_refusal(true, Deterministic), Some(INCOMPLETE_STATES));
        assert_eq!(
            state_refusal(false, Serialized),
            Some(STATES_NOT_FRAME_EXACT)
        );
        assert_eq!(
            state_refusal(true, Serialized),
            Some(STATES_NOT_FRAME_EXACT)
        );
    }
}
