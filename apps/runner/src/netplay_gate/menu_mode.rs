//! The non-pausing menu rules (F5) as pure decisions, used by `pause_glue` and
//! `netplay_glue`: in netplay the open menu never pauses (frames keep ticking, no
//! achievement idle, no redraw sleep), opening / closing never resets pacing or
//! drops audio, and the local port sends neutral input while the menu is open.

use input_mapping::PortState;

/// What a menu open / close does besides showing the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuTransition {
    /// `LoopTiming::resume_normal_speed` (pacer reset + audio prefill / drop).
    pub reset_pacing: bool,
    /// "Paused" in the title, achievement idle and the paused redraw sleep.
    pub paused: bool,
    /// Pad buttons still held at close are hidden from the core until released.
    pub swallow_held_buttons: bool,
}

/// The transition for local play (`netplay = false`) or a netplay session.
pub fn menu_transition(netplay: bool) -> MenuTransition {
    MenuTransition {
        reset_pacing: !netplay,
        paused: !netplay,
        swallow_held_buttons: !netplay,
    }
}

/// Whether this redraw runs no frames (menu open in local play only).
pub fn menu_pauses(menu_open: bool, netplay: bool) -> bool {
    menu_open && menu_transition(netplay).paused
}

/// The port this player sends: neutral while the menu is open (its navigation
/// never reaches the game), else the scripted test port, else the live devices.
pub fn local_port(
    menu_open: bool,
    scripted: Option<PortState>,
    live: impl FnOnce() -> PortState,
) -> PortState {
    match (menu_open, scripted) {
        (true, _) => PortState::default(),
        (false, Some(port)) => port,
        (false, None) => live(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netplay_menu_never_pauses_or_resets_pacing() {
        assert!(
            !menu_pauses(true, true),
            "frames keep ticking under the menu"
        );
        assert!(menu_pauses(true, false), "local play pauses as before");
        assert!(!menu_pauses(false, false));
        let netplay = menu_transition(true);
        assert!(!netplay.reset_pacing && !netplay.paused && !netplay.swallow_held_buttons);
        let local = menu_transition(false);
        assert!(local.reset_pacing && local.paused && local.swallow_held_buttons);
    }

    #[test]
    fn menu_navigation_never_reaches_the_local_port() {
        let pressed = PortState {
            buttons: 1 << 4, // D-pad Up used for menu navigation
            ..PortState::default()
        };
        assert_eq!(local_port(true, None, || pressed), PortState::default());
        assert_eq!(local_port(false, None, || pressed), pressed);
        let scripted = PortState {
            buttons: 1,
            ..PortState::default()
        };
        assert_eq!(local_port(false, Some(scripted), || pressed), scripted);
        assert_eq!(
            local_port(true, Some(scripted), || pressed),
            PortState::default()
        );
    }
}
