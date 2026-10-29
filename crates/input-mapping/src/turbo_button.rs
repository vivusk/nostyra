//! Auto-fire for the A button of player 1 (`turbo_a_toggle` hotkey, default P):
//! while on, A is pressed for [`TURBO_HALF_PERIOD_FRAMES`] frames, released for as
//! many, and so on. The phase counts **real-timeline** frames only (the runner
//! bumps [`TurboButton::frame_clock`] once per real frame), so run-ahead's ahead
//! frames all see the next real frame's phase instead of speeding the pattern up, and
//! fast-forward keeps the same per-frame rhythm (dialogs still advance).

use std::cell::Cell;
use std::rc::Rc;

use crate::joypad_state_snapshot::{JoypadButton, JoypadStateSnapshot};

/// Frames pressed, then frames released: 3 + 3 ≈ 10 presses per second at 60 fps.
/// A release of several frames lets games that only react to a new press (most
/// "confirm" handlers) see every one.
pub const TURBO_HALF_PERIOD_FRAMES: u64 = 3;

/// Player 1 auto-fire of RetroPad A; off at session start.
#[derive(Debug, Default)]
pub struct TurboButton {
    enabled: bool,
    /// Real-timeline frames since start, shared with the runner's frame hooks.
    frames: Rc<Cell<u64>>,
}

impl TurboButton {
    /// Switches auto-fire on/off; returns the new state. Starts each "on" with a press.
    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.frames.set(0);
        self.enabled
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Counter the runner increments once per real-timeline frame.
    pub fn frame_clock(&self) -> Rc<Cell<u64>> {
        Rc::clone(&self.frames)
    }

    /// ORs A into port 0 when auto-fire is on and the phase is "pressed".
    pub(crate) fn press_into(&self, snapshot: &mut JoypadStateSnapshot) {
        if self.enabled && (self.frames.get() / TURBO_HALF_PERIOD_FRAMES).is_multiple_of(2) {
            snapshot.press_mask(0, JoypadButton::A.mask());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_pressed(turbo: &TurboButton) -> bool {
        let mut snapshot = JoypadStateSnapshot::default();
        turbo.press_into(&mut snapshot);
        snapshot.state(0, u32::from(JoypadButton::A as u8)) != 0
    }

    #[test]
    fn presses_three_frames_then_releases_three_while_on() {
        let mut turbo = TurboButton::default();
        let clock = turbo.frame_clock();
        assert!(!a_pressed(&turbo), "off by default");
        assert!(turbo.toggle());
        let pattern: Vec<bool> = (0..12)
            .map(|frame| {
                clock.set(frame);
                a_pressed(&turbo)
            })
            .collect();
        let on = [true, true, true, false, false, false];
        assert_eq!(pattern, [on, on].concat());
        assert!(!turbo.toggle());
        clock.set(0);
        assert!(!a_pressed(&turbo), "off again");
    }

    #[test]
    fn switching_on_restarts_with_a_press() {
        let mut turbo = TurboButton::default();
        let clock = turbo.frame_clock();
        clock.set(4); // would be a release frame
        turbo.toggle();
        assert_eq!(clock.get(), 0);
        assert!(a_pressed(&turbo));
        let mut snapshot = JoypadStateSnapshot::default();
        turbo.press_into(&mut snapshot);
        assert_eq!(snapshot.state(1, 8), 0, "player 2 untouched");
    }
}
