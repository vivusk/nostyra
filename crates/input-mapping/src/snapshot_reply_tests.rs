use super::*;
use crate::analog_stick_input::{StickDirection, ANALOG_MAX};
use crate::joypad_state_snapshot::JoypadButton;

#[test]
fn analog_device_reads_sticks_and_joypad_reads_buttons() {
    let mut snap = JoypadStateSnapshot::default();
    snap.press(0, JoypadButton::B);
    snap.set_axes(0, [9_000, 0, 0, 0]);
    snap.hold_sticks(0, StickDirection::RightDown.mask());
    assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_JOYPAD, 0, 0), 1);
    assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, 0, 0), 9_000);
    assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, 1, 1), ANALOG_MAX);
    assert_eq!(reply(&snap, None, 0, 3, 0, 0), 0, "mouse");
    assert_eq!(
        reply(&snap, None, 0, RETRO_DEVICE_POINTER, 0, 15),
        0,
        "no touch screen"
    );
}

#[test]
fn pointer_answers_port_zero_of_touch_screen_systems() {
    let snap = JoypadStateSnapshot::default();
    let pointer = PointerState {
        x: 1_000,
        y: -2_000,
        pressed: true,
        offscreen: false,
    };
    let touch = Some(&pointer);
    assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 0), 1_000);
    assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 1), -2_000);
    assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 2), 1);
    assert_eq!(
        reply(&snap, touch, 1, RETRO_DEVICE_POINTER, 0, 2),
        0,
        "port 1"
    );
    assert_eq!(
        reply(&snap, touch, 0, RETRO_DEVICE_JOYPAD, 0, 0),
        0,
        "pad unaffected"
    );
}

#[test]
fn systems_without_analog_sticks_reply_centred() {
    // The pad manager never writes axes for them and their configs drop the
    // stick actions, so the snapshot stays centred.
    let mut snap = JoypadStateSnapshot::default();
    snap.press_mask(0, JoypadButton::Right.mask());
    for (index, id) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, index, id), 0);
    }
}
