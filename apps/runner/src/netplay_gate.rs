//! The one runtime value every netplay rule keys off (M20 phase 04), same sharing
//! pattern as `hardcore_gate`: the seat of this runner in a running netplay session
//! (`Off` outside netplay). The loop refuses lockstep-breaking actions with the
//! texts in `refusals`; the pause menu turns into the non-pausing netplay menu.

use std::cell::Cell;
use std::rc::Rc;

use input_mapping::RunnerHotkey;

use crate::pause_menu::MenuAction;

pub(crate) mod menu_mode;
pub(crate) mod refusals;

/// This runner's seat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetplaySeat {
    #[default]
    Off,
    Host,
    Player,
    Spectator,
}

/// Something the player tried that may break lockstep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatedAction {
    Rewind,
    FastForward,
    LoadState,
    SaveState,
    Reset,
    DiscSwap,
    RunAhead,
}

/// Shared netplay seat of one session (clones see the same value).
#[derive(Clone, Debug, Default)]
pub struct NetplayGate(Rc<Cell<NetplaySeat>>);

impl NetplayGate {
    pub fn new(seat: NetplaySeat) -> Self {
        Self(Rc::new(Cell::new(seat)))
    }

    /// Whether a netplay session runs (any seat).
    pub fn active(&self) -> bool {
        self.0.get() != NetplaySeat::Off
    }

    pub fn seat(&self) -> NetplaySeat {
        self.0.get()
    }

    /// Why `action` is refused right now (`None`: allowed).
    pub fn refusal(&self, action: GatedAction) -> Option<&'static str> {
        refusals::refusal(self.seat(), action)
    }

    /// Refusal of a pause-menu action (load, reset, disc, save, run-ahead).
    pub fn refusal_of_menu(&self, action: &MenuAction) -> Option<&'static str> {
        let gated = match action {
            MenuAction::LoadSlot(_) => GatedAction::LoadState,
            MenuAction::SaveSlot(_) => GatedAction::SaveState,
            MenuAction::ResetCore => GatedAction::Reset,
            MenuAction::SwapDisc(_) => GatedAction::DiscSwap,
            MenuAction::SetRunAhead(_) => GatedAction::RunAhead,
            MenuAction::SetRewind(_) => GatedAction::Rewind,
            _ => return None,
        };
        self.refusal(gated)
    }

    /// Refusal of a press hotkey (F1 save, F3 load).
    pub fn refusal_of_hotkey(&self, hotkey: RunnerHotkey) -> Option<&'static str> {
        match hotkey {
            RunnerHotkey::SaveState => self.refusal(GatedAction::SaveState),
            RunnerHotkey::LoadState => self.refusal(GatedAction::LoadState),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_the_seat() {
        let gate = NetplayGate::new(NetplaySeat::Player);
        let holder = gate.clone();
        assert!(holder.active());
        assert_eq!(holder.seat(), NetplaySeat::Player);
        assert!(!NetplayGate::default().active(), "outside netplay");
        assert_eq!(NetplayGate::default().refusal(GatedAction::Rewind), None);
    }
}
