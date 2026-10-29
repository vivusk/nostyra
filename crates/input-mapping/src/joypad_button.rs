//! RetroPad buttons: discriminant = `RETRO_DEVICE_ID_JOYPAD_*` id, config action
//! name ↔ button.

/// RetroPad buttons (the SNES pad, the second shoulder pair, the stick clicks);
/// discriminant = `RETRO_DEVICE_ID_JOYPAD_*` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JoypadButton {
    B = 0,
    Y = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
    A = 8,
    X = 9,
    L = 10,
    R = 11,
    L2 = 12,
    R2 = 13,
    L3 = 14,
    R3 = 15,
}

impl JoypadButton {
    /// Bit for this button in a port bitmask.
    pub const fn mask(self) -> u16 {
        1 << self as u8
    }

    /// The button a config action name (`config_layers::PAD_ACTIONS`) stands for.
    pub fn from_action(name: &str) -> Option<Self> {
        Some(match name {
            "b" => Self::B,
            "y" => Self::Y,
            "select" => Self::Select,
            "start" => Self::Start,
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            "a" => Self::A,
            "x" => Self::X,
            "l" => Self::L,
            "r" => Self::R,
            "l2" => Self::L2,
            "r2" => Self::R2,
            "l3" => Self::L3,
            "r3" => Self::R3,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_config_action_maps_to_a_distinct_button() {
        let mask = config_layers::PAD_ACTIONS.iter().fold(0u16, |mask, name| {
            let bit = JoypadButton::from_action(name).map_or(0, JoypadButton::mask);
            assert_eq!(mask & bit, 0, "{name} shares a bit");
            mask | bit
        });
        assert_eq!(mask, 0xFFFF, "all 16 RetroPad ids");
        assert_eq!(JoypadButton::from_action("turbo"), None);
    }

    #[test]
    fn stick_clicks_are_retropad_ids_14_and_15() {
        assert_eq!(JoypadButton::from_action("l3"), Some(JoypadButton::L3));
        assert_eq!(JoypadButton::from_action("r3"), Some(JoypadButton::R3));
        assert_eq!(JoypadButton::L3.mask(), 1 << 14);
        assert_eq!(JoypadButton::R3.mask(), 1 << 15);
    }
}
