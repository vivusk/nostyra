//! Runner hotkeys (system-architecture §3.5). Handled by the runner, never
//! forwarded to the core. Which inputs trigger them comes from the config
//! (`hotkeys_keyboard` / `hotkeys_gamepad`, see [`crate::InputBindings`]).

/// One runner hotkey; config names are `config_layers::HOTKEYS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerHotkey {
    SaveState,
    PrevSlot,
    NextSlot,
    LoadState,
    ToggleFullscreen,
    /// Opens the in-game pause menu (quitting moved into it; closing the window
    /// still quits at once). Esc is always bound to it on top of the config.
    PauseMenu,
    /// Held (level-triggered, polled each redraw) for uncapped speed.
    FastForwardHold,
    /// Held (level-triggered) to step back through the rewind history.
    RewindHold,
    /// Press: player 1's A button auto-fires until pressed again (`TurboButton`).
    TurboAToggle,
}

impl RunnerHotkey {
    /// The hotkey a config name stands for (`None` = unknown name).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "pause_menu" => Self::PauseMenu,
            "save_state" => Self::SaveState,
            "load_state" => Self::LoadState,
            "prev_slot" => Self::PrevSlot,
            "next_slot" => Self::NextSlot,
            "toggle_fullscreen" => Self::ToggleFullscreen,
            "fast_forward_hold" => Self::FastForwardHold,
            "rewind_hold" => Self::RewindHold,
            "turbo_a_toggle" => Self::TurboAToggle,
            _ => return None,
        })
    }

    /// Hold hotkeys act while held, not on a press edge.
    pub fn is_hold(self) -> bool {
        matches!(self, Self::FastForwardHold | Self::RewindHold)
    }
}

#[cfg(test)]
mod tests {
    use config_layers::HOTKEYS;

    use super::*;

    #[test]
    fn every_config_hotkey_name_converts_to_a_distinct_hotkey() {
        let hotkeys: Vec<_> = HOTKEYS.iter().map(|n| RunnerHotkey::from_name(n)).collect();
        assert!(hotkeys.iter().all(Option::is_some), "{hotkeys:?}");
        for (i, a) in hotkeys.iter().enumerate() {
            assert!(!hotkeys[i + 1..].contains(a), "{a:?} twice");
        }
        assert_eq!(RunnerHotkey::from_name("quit"), None);
        let holds = hotkeys.iter().flatten().filter(|h| h.is_hold()).count();
        assert_eq!(holds, 2);
    }
}
