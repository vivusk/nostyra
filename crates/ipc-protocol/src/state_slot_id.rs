//! Which save-state files a command means: a user slot `0..=MAX_STATE_SLOT` or the
//! reserved `auto` resume point the runner writes on every clean exit. One type for
//! the runner CLI (`--load-state 3` / `--load-state auto`), the launcher commands
//! (JSON `3` / `"auto"`) and the file names (`slot3.*` / `auto.*`), so the spelling
//! lives in one place.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::save_state_files::MAX_STATE_SLOT;

/// File stem and CLI / JSON spelling of the resume point.
pub const AUTO_STATE_STEM: &str = "auto";

/// A save-state slot: numbered (user, F1/F4 and the slot grid) or the resume point.
/// Serialises as the bare number or the string `"auto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "SlotRepr", into = "SlotRepr")]
pub enum StateSlotId {
    /// `0..=MAX_STATE_SLOT` when built by parsing / deserialising.
    Numbered(u8),
    /// `auto.*`: written on exit, never by F1 or the slot grid.
    Auto,
}

impl StateSlotId {
    /// File stem: `slot3` / `auto`.
    pub fn file_stem(self) -> String {
        match self {
            Self::Numbered(slot) => format!("slot{slot}"),
            Self::Auto => AUTO_STATE_STEM.to_string(),
        }
    }
}

impl fmt::Display for StateSlotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numbered(slot) => write!(f, "{slot}"),
            Self::Auto => f.write_str(AUTO_STATE_STEM),
        }
    }
}

impl FromStr for StateSlotId {
    type Err = String;

    /// `"0"`..`"9"` or `"auto"` (CLI spelling).
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == AUTO_STATE_STEM {
            return Ok(Self::Auto);
        }
        match value.parse::<u8>() {
            Ok(slot) if slot <= MAX_STATE_SLOT => Ok(Self::Numbered(slot)),
            _ => Err(format!(
                "expected a slot number 0-{MAX_STATE_SLOT} or `{AUTO_STATE_STEM}`"
            )),
        }
    }
}

/// Wire form: a number or a string (validated into [`StateSlotId`]).
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SlotRepr {
    Number(u64),
    Name(String),
}

impl TryFrom<SlotRepr> for StateSlotId {
    type Error = String;

    fn try_from(repr: SlotRepr) -> Result<Self, Self::Error> {
        match repr {
            SlotRepr::Number(n) => n.to_string().parse(),
            SlotRepr::Name(name) if name == AUTO_STATE_STEM => Ok(Self::Auto),
            SlotRepr::Name(name) => Err(format!("unknown save-state slot `{name}`")),
        }
    }
}

impl From<StateSlotId> for SlotRepr {
    fn from(id: StateSlotId) -> Self {
        match id {
            StateSlotId::Numbered(slot) => Self::Number(u64::from(slot)),
            StateSlotId::Auto => Self::Name(AUTO_STATE_STEM.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_number_or_auto() {
        let json = |id: StateSlotId| serde_json::to_string(&id).unwrap();
        assert_eq!(json(StateSlotId::Numbered(3)), "3");
        assert_eq!(json(StateSlotId::Auto), "\"auto\"");
        let parse = |s: &str| serde_json::from_str::<StateSlotId>(s);
        assert_eq!(parse("3").unwrap(), StateSlotId::Numbered(3));
        assert_eq!(parse("\"auto\"").unwrap(), StateSlotId::Auto);
        for bad in ["10", "-1", "\"3\"", "\"slot1\"", "null", "1.5"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn cli_spelling_round_trips() {
        for id in [
            StateSlotId::Numbered(0),
            StateSlotId::Numbered(9),
            StateSlotId::Auto,
        ] {
            assert_eq!(id.to_string().parse::<StateSlotId>(), Ok(id));
        }
        for bad in ["10", "", "AUTO", " 1", "x"] {
            assert!(bad.parse::<StateSlotId>().is_err(), "{bad}");
        }
    }

    #[test]
    fn file_stems() {
        assert_eq!(StateSlotId::Numbered(7).file_stem(), "slot7");
        assert_eq!(StateSlotId::Auto.file_stem(), "auto");
    }
}
