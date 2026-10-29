//! Test input for the real-core suites: one port-0 input held for every frame,
//! answered per device (RetroPad button, analog stick, analog trigger, a touch on
//! the DS bottom screen or a raw pointer state), or nothing pressed.

use input_mapping::{
    fraction_to_pointer, PointerState, ANALOG_BUTTON_INDEX, ANALOG_MAX, TRIGGER_MAX,
};
use libretro_host::{RETRO_DEVICE_ANALOG, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_POINTER};

/// One DS screen; the core's default top-bottom frame is 256×384.
const DS_SCREEN: (f64, f64) = (256.0, 192.0);

/// What port 0 holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeldInput {
    /// Nothing pressed, sticks centred.
    None,
    /// RetroPad B (joypad id 0).
    B,
    /// Left stick fully right (analog index 0, X axis).
    LeftStickRight,
    /// Right analog trigger fully pulled (analog index 2, id R2), no digital bit.
    RightTrigger,
    /// Pointer pressed at DS bottom-screen pixel (x, y), top-bottom layout.
    Touch { x: u16, y: u16 },
    /// RetroPad digital button `id` (e.g. 13 = R2, 15 = R3).
    Joypad(u32),
    /// Right stick at (x, y) (analog index 1).
    RightStick(i16, i16),
    /// This pointer state as given (e.g. from the runner's mouse mapping).
    Pointer(PointerState),
}

/// DS bottom-screen pixel → pointer over the whole top-bottom frame (pixel centre,
/// so the core's mapping back lands on the same pixel).
fn ds_touch(x: u16, y: u16) -> PointerState {
    let fx = (f64::from(x) + 0.5) / DS_SCREEN.0;
    let fy = (DS_SCREEN.1 + f64::from(y) + 0.5) / (DS_SCREEN.1 * 2.0);
    PointerState {
        x: fraction_to_pointer(fx),
        y: fraction_to_pointer(fy),
        pressed: true,
        offscreen: false,
    }
}

impl libretro_host::InputSource for HeldInput {
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        if port != 0 {
            return 0;
        }
        match (*self, device, index, id) {
            (Self::B, RETRO_DEVICE_JOYPAD, _, 0) => 1,
            (Self::LeftStickRight, RETRO_DEVICE_ANALOG, 0, 0) => ANALOG_MAX,
            (Self::RightTrigger, RETRO_DEVICE_ANALOG, ANALOG_BUTTON_INDEX, 13) => TRIGGER_MAX,
            (Self::Touch { x, y }, RETRO_DEVICE_POINTER, _, _) => ds_touch(x, y).reply(index, id),
            (Self::Joypad(button), RETRO_DEVICE_JOYPAD, _, id) if id == button => 1,
            (Self::RightStick(x, _), RETRO_DEVICE_ANALOG, 1, 0) => x,
            (Self::RightStick(_, y), RETRO_DEVICE_ANALOG, 1, 1) => y,
            (Self::Pointer(state), RETRO_DEVICE_POINTER, _, _) => state.reply(index, id),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use libretro_host::InputSource;

    use super::*;

    #[test]
    fn each_variant_answers_only_its_device() {
        let mut b = HeldInput::B;
        assert_eq!(b.state(0, RETRO_DEVICE_JOYPAD, 0, 0), 1);
        assert_eq!(b.state(1, RETRO_DEVICE_JOYPAD, 0, 0), 0, "port 0 only");
        assert_eq!(b.state(0, RETRO_DEVICE_ANALOG, 0, 0), 0);
        let mut stick = HeldInput::LeftStickRight;
        assert_eq!(stick.state(0, RETRO_DEVICE_ANALOG, 0, 0), ANALOG_MAX);
        assert_eq!(stick.state(0, RETRO_DEVICE_ANALOG, 0, 1), 0, "Y centred");
        assert_eq!(stick.state(0, RETRO_DEVICE_JOYPAD, 0, 0), 0);
        assert_eq!(HeldInput::None.state(0, RETRO_DEVICE_JOYPAD, 0, 0), 0);
        let mut rt = HeldInput::RightTrigger;
        assert_eq!(rt.state(0, RETRO_DEVICE_ANALOG, 2, 13), TRIGGER_MAX);
        assert_eq!(rt.state(0, RETRO_DEVICE_ANALOG, 2, 12), 0, "left trigger");
        assert_eq!(rt.state(0, RETRO_DEVICE_JOYPAD, 0, 13), 0, "no digital bit");
        assert_eq!(b.state(0, RETRO_DEVICE_POINTER, 0, 2), 0, "no touch");
        let mut r3 = HeldInput::Joypad(15);
        assert_eq!(r3.state(0, RETRO_DEVICE_JOYPAD, 0, 15), 1);
        assert_eq!(r3.state(0, RETRO_DEVICE_JOYPAD, 0, 13), 0);
        let mut right = HeldInput::RightStick(100, -200);
        assert_eq!(right.state(0, RETRO_DEVICE_ANALOG, 1, 0), 100);
        assert_eq!(right.state(0, RETRO_DEVICE_ANALOG, 1, 1), -200);
        assert_eq!(right.state(0, RETRO_DEVICE_ANALOG, 0, 0), 0, "left stick");
        let mut hover = HeldInput::Pointer(PointerState::default());
        assert_eq!(hover.state(0, RETRO_DEVICE_POINTER, 0, 2), 0, "not pressed");
    }

    #[test]
    fn touch_maps_bottom_screen_pixels_over_the_whole_frame() {
        let mut centre = HeldInput::Touch { x: 128, y: 96 };
        // Spike basis: frame (0.5, 0.75) = DS (128, 96); pixel centre → just past it.
        let x = centre.state(0, RETRO_DEVICE_POINTER, 0, 0);
        let y = centre.state(0, RETRO_DEVICE_POINTER, 0, 1);
        assert!((0..=200).contains(&x), "{x}");
        assert!((16_384..=16_500).contains(&y), "{y}");
        assert_eq!(centre.state(0, RETRO_DEVICE_POINTER, 0, 2), 1, "pressed");
        assert_eq!(
            centre.state(0, RETRO_DEVICE_POINTER, 1, 2),
            0,
            "second touch"
        );
        assert_eq!(centre.state(1, RETRO_DEVICE_POINTER, 0, 2), 0, "port 1");
        assert_eq!(centre.state(0, RETRO_DEVICE_JOYPAD, 0, 0), 0);
        let mut corner = HeldInput::Touch { x: 0, y: 0 };
        assert!(
            corner.state(0, RETRO_DEVICE_POINTER, 0, 1) > 0,
            "below the top screen"
        );
    }
}
