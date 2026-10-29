//! Analog sticks for systems that have them (`SystemSpec.analog_sticks`): the
//! `RETRO_DEVICE_ANALOG` reply combines the raw SDL axes with keys / buttons bound
//! to the stick actions (`lstick_*` / `rstick_*`). A held direction gives full
//! deflection and wins over the axis; opposite directions held together cancel to
//! 0. No host deadzone: the core applies its own (Mupen64Plus-Next: 15 % radial).

/// libretro analog range is ±0x7FFF (SDL's −32768 is clamped to it).
pub const ANALOG_MAX: i16 = 0x7FFF;

/// One stick direction; discriminant = bit in a port's stick-hold mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StickDirection {
    LeftUp = 0,
    LeftDown = 1,
    LeftLeft = 2,
    LeftRight = 3,
    RightUp = 4,
    RightDown = 5,
    RightLeft = 6,
    RightRight = 7,
}

impl StickDirection {
    /// Bit for this direction in a port's stick-hold mask.
    pub const fn mask(self) -> u8 {
        1 << self as u8
    }

    /// The direction a config action name (`config_layers::PAD_ACTIONS`) stands for.
    pub fn from_action(name: &str) -> Option<Self> {
        Some(match name {
            "lstick_up" => Self::LeftUp,
            "lstick_down" => Self::LeftDown,
            "lstick_left" => Self::LeftLeft,
            "lstick_right" => Self::LeftRight,
            "rstick_up" => Self::RightUp,
            "rstick_down" => Self::RightDown,
            "rstick_left" => Self::RightLeft,
            "rstick_right" => Self::RightRight,
            _ => return None,
        })
    }
}

/// libretro `input_state` answer for `RETRO_DEVICE_ANALOG`: `axes` = raw
/// `[left x, left y, right x, right y]` (+y = down, as SDL and libretro agree),
/// `holds` = held stick directions. Index 0 left / 1 right stick, id 0 X / 1 Y;
/// anything else (index 2 = analog buttons) → 0.
pub fn analog_value(axes: [i16; 4], holds: u8, index: u32, id: u32) -> i16 {
    if index > 1 || id > 1 {
        return 0;
    }
    // Per stick the bits are up, down, left, right: X uses left/right, Y up/down.
    let base = index * 4 + if id == 0 { 2 } else { 0 };
    let held = |bit: u32| holds & (1 << bit) != 0;
    axis_value(axes[(index * 2 + id) as usize], held(base), held(base + 1))
}

/// One axis: a held direction wins (full deflection, opposites cancel), else
/// the raw axis clamped to the libretro range.
pub fn axis_value(axis: i16, negative_held: bool, positive_held: bool) -> i16 {
    match (negative_held, positive_held) {
        (false, false) => axis.max(-ANALOG_MAX),
        (true, false) => -ANALOG_MAX,
        (false, true) => ANALOG_MAX,
        (true, true) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_key_wins_and_opposites_cancel() {
        assert_eq!(axis_value(1_000, false, true), ANALOG_MAX);
        assert_eq!(axis_value(1_000, true, false), -ANALOG_MAX);
        assert_eq!(axis_value(20_000, true, true), 0);
        assert_eq!(axis_value(1_234, false, false), 1_234);
        assert_eq!(axis_value(i16::MIN, false, false), -ANALOG_MAX, "clamped");
    }

    #[test]
    fn analog_reply_per_stick_and_axis() {
        let axes = [100, -200, 300, i16::MIN];
        assert_eq!(analog_value(axes, 0, 0, 0), 100);
        assert_eq!(analog_value(axes, 0, 0, 1), -200);
        assert_eq!(analog_value(axes, 0, 1, 0), 300);
        assert_eq!(analog_value(axes, 0, 1, 1), -ANALOG_MAX);
        let right = StickDirection::LeftRight.mask();
        assert_eq!(analog_value(axes, right, 0, 0), ANALOG_MAX);
        assert_eq!(analog_value(axes, right, 0, 1), -200, "Y untouched");
        assert_eq!(
            analog_value(axes, right, 1, 0),
            300,
            "other stick untouched"
        );
        let c_up = StickDirection::RightUp.mask();
        assert_eq!(analog_value(axes, c_up, 1, 1), -ANALOG_MAX, "up = −Y");
        let c_down = StickDirection::RightDown.mask();
        assert_eq!(analog_value([0; 4], c_down, 1, 1), ANALOG_MAX);
        let left = StickDirection::LeftLeft.mask();
        assert_eq!(analog_value(axes, left | right, 0, 0), 0);
        assert_eq!(analog_value(axes, 0xFF, 2, 0), 0, "analog buttons");
        assert_eq!(analog_value(axes, 0xFF, 0, 2), 0);
    }

    #[test]
    fn every_stick_action_has_its_own_bit() {
        let mask = config_layers::PAD_ACTIONS.iter().fold(0u8, |mask, name| {
            let bit = StickDirection::from_action(name).map_or(0, StickDirection::mask);
            assert_eq!(mask & bit, 0, "{name} shares a bit");
            mask | bit
        });
        assert_eq!(mask, 0xFF);
    }
}
