use super::*;
use crate::analog_stick_input::ANALOG_MAX;
use crate::analog_trigger_input::TRIGGER_MAX;

fn mask(buttons: &[JoypadButton]) -> u16 {
    buttons.iter().fold(0, |m, b| m | b.mask())
}

#[test]
fn digital_port_round_trips_losslessly() {
    let state = PortState {
        buttons: mask(&[JoypadButton::A, JoypadButton::Start, JoypadButton::R3]),
        ..PortState::default()
    };
    let wire = netplay_input_from_port(state, false);
    assert_eq!(wire.axes, [0; 4]);
    assert_eq!(port_from_netplay_input(wire), state);
    let idle = netplay_input_from_port(PortState::default(), true);
    assert_eq!(idle, NetplayInput::default());
}

#[test]
fn analog_port_carries_axes_with_holds_resolved() {
    let state = PortState {
        buttons: JoypadButton::B.mask(),
        axes: [1_000, -2_000, i16::MIN, 300],
        stick_holds: StickDirection::RightUp.mask(),
        ..PortState::default()
    };
    let wire = netplay_input_from_port(state, true);
    assert_eq!(wire.axes, [1_000, -2_000, -ANALOG_MAX, -ANALOG_MAX]);
    let back = port_from_netplay_input(wire);
    assert_eq!(back.axes, wire.axes);
    assert_eq!(back.buttons, JoypadButton::B.mask(), "no D-pad folding");
}

#[test]
fn digital_systems_drop_axes_and_fold_left_stick_holds_into_the_dpad() {
    // (hold, expected D-pad bit): right-stick holds have no D-pad equivalent.
    let table = [
        (StickDirection::LeftUp, JoypadButton::Up.mask()),
        (StickDirection::LeftDown, JoypadButton::Down.mask()),
        (StickDirection::LeftLeft, JoypadButton::Left.mask()),
        (StickDirection::LeftRight, JoypadButton::Right.mask()),
        (StickDirection::RightUp, 0),
        (StickDirection::RightRight, 0),
    ];
    for (hold, bit) in table {
        let state = PortState {
            axes: [9_000; 4],
            stick_holds: hold.mask(),
            ..PortState::default()
        };
        let wire = netplay_input_from_port(state, false);
        assert_eq!((wire.buttons, wire.axes), (bit, [0; 4]), "{hold:?}");
    }
}

#[test]
fn trigger_pressure_past_half_travel_becomes_l2_r2_bits() {
    let both = mask(&[JoypadButton::L2, JoypadButton::R2]);
    let table = [
        ([0, 0], 0),
        ([TRIGGER_MAX / 4, 0], 0),
        ([TRIGGER_MAX, 0], JoypadButton::L2.mask()),
        ([0, TRIGGER_MAX], JoypadButton::R2.mask()),
        ([TRIGGER_MAX; 2], both),
    ];
    for (triggers, bits) in table {
        let state = PortState {
            triggers,
            ..PortState::default()
        };
        let wire = netplay_input_from_port(state, true);
        assert_eq!(wire.buttons, bits, "{triggers:?}");
    }
}
