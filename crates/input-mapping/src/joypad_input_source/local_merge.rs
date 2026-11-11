//! Netplay "local devices = my port": the keyboard (port 0), every local pad (one
//! port each) and turbo collapse into the one port this player sends.

use crate::joypad_state_snapshot::{JoypadStateSnapshot, PortState, PORT_COUNT};

/// OR of every port's buttons and held stick directions, the first port with a
/// deflected stick for the raw axes, the strongest pressure per trigger.
pub(super) fn collapse_ports(snapshot: &JoypadStateSnapshot) -> PortState {
    let mut merged = PortState::default();
    for port in 0..PORT_COUNT {
        let state = snapshot.port(port);
        merged.buttons |= state.buttons;
        merged.stick_holds |= state.stick_holds;
        if merged.axes == [0; 4] {
            merged.axes = state.axes;
        }
        merged.triggers = [
            merged.triggers[0].max(state.triggers[0]),
            merged.triggers[1].max(state.triggers[1]),
        ];
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog_stick_input::StickDirection;
    use crate::joypad_state_snapshot::JoypadButton;

    #[test]
    fn keyboard_and_two_pads_merge_into_one_port() {
        let mut snap = JoypadStateSnapshot::default();
        // Keyboard on port 0, pad 1 on port 0 too, pad 2 on port 1.
        snap.press(0, JoypadButton::Left);
        snap.hold_sticks(0, StickDirection::LeftUp.mask());
        snap.press(0, JoypadButton::A);
        snap.press(1, JoypadButton::Start);
        snap.set_axes(1, [1_000, 0, 0, 0]);
        snap.raise_triggers(1, [200, 0]);
        snap.set_axes(2, [9_000, 9_000, 0, 0]);
        snap.raise_triggers(2, [100, 300]);
        let merged = collapse_ports(&snap);
        let buttons = JoypadButton::Left.mask() | JoypadButton::A.mask();
        assert_eq!(merged.buttons, buttons | JoypadButton::Start.mask());
        assert_eq!(merged.stick_holds, StickDirection::LeftUp.mask());
        assert_eq!(merged.axes, [1_000, 0, 0, 0], "first deflected stick");
        assert_eq!(merged.triggers, [200, 300]);
    }

    #[test]
    fn nothing_held_is_the_default_port() {
        let merged = collapse_ports(&JoypadStateSnapshot::default());
        assert_eq!(merged, PortState::default());
    }
}
