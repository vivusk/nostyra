//! `--netplay-scripted-input <pattern>` (test flag): a [`HeldInput`] from text and
//! the netplay port it stands for. Patterns: `none`, `b`, `joypad:<id>` (RetroPad id
//! 0..=15), `stick-right` (left stick full right), `trigger-right` (R2 fully pulled).

use input_mapping::{PortState, ANALOG_MAX, TRIGGER_MAX};

use super::HeldInput;

/// Parses a pattern (clap value parser: an `Err` is a usage error, exit 2).
pub fn parse_pattern(text: &str) -> Result<HeldInput, String> {
    let held = match text {
        "none" => HeldInput::None,
        "b" => HeldInput::B,
        "stick-right" => HeldInput::LeftStickRight,
        "trigger-right" => HeldInput::RightTrigger,
        _ => match text.strip_prefix("joypad:").map(str::parse::<u32>) {
            Some(Ok(id)) if id < 16 => HeldInput::Joypad(id),
            _ => {
                return Err("expected none, b, joypad:<0-15>, stick-right or trigger-right".into())
            }
        },
    };
    Ok(held)
}

impl HeldInput {
    /// The local player's port while this is held (pointer / touch: nothing, netplay
    /// carries no pointer).
    pub fn port_state(self) -> PortState {
        let mut port = PortState::default();
        match self {
            Self::B => port.buttons = 1,
            Self::Joypad(id) if id < 16 => port.buttons = 1 << id,
            Self::LeftStickRight => port.axes[0] = ANALOG_MAX,
            Self::RightStick(x, y) => (port.axes[2], port.axes[3]) = (x, y),
            Self::RightTrigger => port.triggers[1] = TRIGGER_MAX,
            _ => {}
        }
        port
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_parse_and_map_to_ports() {
        assert_eq!(parse_pattern("none"), Ok(HeldInput::None));
        assert_eq!(parse_pattern("b"), Ok(HeldInput::B));
        assert_eq!(parse_pattern("joypad:13"), Ok(HeldInput::Joypad(13)));
        assert_eq!(parse_pattern("stick-right"), Ok(HeldInput::LeftStickRight));
        assert_eq!(parse_pattern("trigger-right"), Ok(HeldInput::RightTrigger));
        for bad in ["joypad:x", "joypad:16", "", "B", "start"] {
            assert!(parse_pattern(bad).is_err(), "{bad}");
        }
        let b = HeldInput::B.port_state();
        assert_eq!(
            b,
            PortState {
                buttons: 1,
                ..PortState::default()
            },
            "only B"
        );
        assert_eq!(HeldInput::None.port_state(), PortState::default());
        assert_eq!(HeldInput::Joypad(13).port_state().buttons, 1 << 13);
        assert_eq!(
            HeldInput::LeftStickRight.port_state().axes,
            [ANALOG_MAX, 0, 0, 0]
        );
        assert_eq!(
            HeldInput::RightTrigger.port_state().triggers,
            [0, TRIGGER_MAX]
        );
    }
}
