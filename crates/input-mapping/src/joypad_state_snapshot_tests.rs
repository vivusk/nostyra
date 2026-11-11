use super::*;

#[test]
fn press_and_query_by_retro_id() {
    let mut snap = JoypadStateSnapshot::default();
    snap.press(0, JoypadButton::A);
    snap.press(1, JoypadButton::Start);
    assert_eq!(snap.state(0, 8), 1);
    assert_eq!(snap.state(0, 0), 0);
    assert_eq!(snap.state(1, 3), 1);
    assert_eq!(snap.port_mask(0), 1 << 8);
}

#[test]
fn analog_axes_and_holds_combine_per_port() {
    use crate::analog_stick_input::{StickDirection, ANALOG_MAX};
    let mut snap = JoypadStateSnapshot::default();
    assert_eq!(snap.analog_state(0, 0, 0), 0, "no stick → centred");
    snap.set_axes(0, [5_000, 0, 0, -7_000]);
    snap.hold_sticks(1, StickDirection::LeftRight.mask());
    assert_eq!(snap.analog_state(0, 0, 0), 5_000);
    assert_eq!(snap.analog_state(0, 1, 1), -7_000);
    assert_eq!(snap.analog_state(1, 0, 0), ANALOG_MAX);
    assert_eq!(snap.state(1, 7), 0, "a stick hold is not a D-pad press");
}

#[test]
fn out_of_range_is_harmless() {
    let mut snap = JoypadStateSnapshot::default();
    snap.press(7, JoypadButton::B);
    assert_eq!(snap, JoypadStateSnapshot::default());
    assert_eq!(snap.state(5, 0), 0);
    assert_eq!(snap.state(0, 300), 0);
    assert_eq!(snap.analog_state(5, 0, 0), 0);
}

#[test]
fn ports_two_and_three_exist_and_port_state_round_trips() {
    let mut snap = JoypadStateSnapshot::default();
    let state = PortState {
        buttons: JoypadButton::Start.mask(),
        axes: [1, -2, 3, -4],
        triggers: [5, 6],
        stick_holds: 0b1000_0001,
    };
    snap.set_port(3, state);
    assert_eq!(snap.port(3), state);
    assert_eq!(snap.state(3, 3), 1);
    assert_eq!(snap.axes(3), [1, -2, 3, -4]);
    assert_eq!(snap.port(2), PortState::default());
    snap.set_port(PORT_COUNT, state);
    assert_eq!(snap.port(PORT_COUNT), PortState::default(), "out of range");
    assert_eq!(snap.state(PORT_COUNT, 3), 0);
}
