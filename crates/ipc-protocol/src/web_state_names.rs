//! Save-state file stems of the web edition (EmulatorJS builds of the cores). They
//! live in the same game folder as the desktop states but never share a name with
//! them: `web-slot0..9` and `web-auto`. Desktop listings only enumerate
//! [`StateSlotId`] stems (`slotN` / `auto`), and the web only these, so neither app
//! ever offers the other's (incompatible) states.

use crate::state_slot_id::StateSlotId;

/// SRAM the web writes instead of `save.srm` when its core's SRAM length differs from
/// the desktop file's (phase 00 D7: an unproven cartridge board); the desktop never
/// reads it, and the web loads it before `save.srm`.
pub const WEB_SRAM_FILE_NAME: &str = "save.web.srm";

/// Prefix that sets web state stems apart from the desktop ones.
pub const WEB_STATE_PREFIX: &str = "web-";

/// `web-slot3` / `web-auto`: the file stem of a web save state for `id`.
pub fn web_state_stem(id: StateSlotId) -> String {
    format!("{WEB_STATE_PREFIX}{}", id.file_stem())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_stems_never_collide_with_desktop_stems() {
        assert_eq!(web_state_stem(StateSlotId::Numbered(3)), "web-slot3");
        assert_eq!(web_state_stem(StateSlotId::Auto), "web-auto");
        for slot in 0..=crate::MAX_STATE_SLOT {
            let web = web_state_stem(StateSlotId::Numbered(slot));
            assert_ne!(web, StateSlotId::Numbered(slot).file_stem());
            assert!(web.parse::<StateSlotId>().is_err(), "{web}");
            let stem = web.trim_start_matches(WEB_STATE_PREFIX);
            assert_eq!(stem, StateSlotId::Numbered(slot).file_stem());
        }
        assert!(web_state_stem(StateSlotId::Auto)
            .parse::<StateSlotId>()
            .is_err());
    }
}
