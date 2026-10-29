//! Messages on the runner's stdout (newline-delimited JSON, only with `--ipc`).
//!
//! Cores may print to stdout too (snes9x `printf`, block-buffered, often without a
//! trailing newline), so every event is written as `"\n" + json + "\n"` and the
//! decoder starts at the first `{"type"` of a line, ignoring anything before it.

use serde::{Deserialize, Serialize};

/// Runner → launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RunnerEvent {
    /// Core and game loaded (windowed mode: game window open). A play session counts
    /// only after this event.
    Started { core: String, rom: String },
    /// Clean end of a started session, sent after the final SRAM flush.
    Stopped { play_seconds: u64 },
    /// Fatal error, sent just before a non-zero exit.
    Error { message: String },
    /// RetroAchievements set loaded for this game (`ra_game_id` = RA's id; counts of
    /// core achievements). The snapshot file is written just before.
    AchievementsLoaded {
        ra_game_id: u32,
        unlocked: u32,
        total: u32,
        hardcore: bool,
    },
    /// An achievement unlocked this session (snapshot updated just before).
    AchievementUnlocked { id: u32, hardcore: bool },
    /// The server rejected the stored token: the launcher logs the user out.
    AchievementsLoginExpired,
    /// Answer to `--probe-core`.
    CoreInfo {
        name: String,
        version: String,
        api_version: u32,
    },
}

/// Launcher → runner. Reserved for in-game control (M2+); not sent in M1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LauncherCommand {
    Pause,
    Resume,
    SaveState { slot: u8 },
    Quit,
}

/// Marker the decoder searches for (serde writes the tag first).
const EVENT_PREFIX: &str = "{\"type\"";

/// One stdout write: leading newline isolates the event from unterminated core output.
pub fn encode_event_line(event: &RunnerEvent) -> String {
    // Serializing these plain enums cannot fail (no maps with non-string keys).
    let json = serde_json::to_string(event).unwrap_or_default();
    format!("\n{json}\n")
}

/// Parses an event from a stdout line; `None` for core noise or unknown events.
pub fn decode_event_line(line: &str) -> Option<RunnerEvent> {
    let start = line.find(EVENT_PREFIX)?;
    serde_json::from_str(line[start..].trim_end()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_events() -> Vec<RunnerEvent> {
        vec![
            RunnerEvent::Started {
                core: "Snes9x".into(),
                rom: "E:/ROMs/Game \"1\".sfc".into(),
            },
            RunnerEvent::Stopped { play_seconds: 42 },
            RunnerEvent::Error {
                message: "line\nbreak".into(),
            },
            RunnerEvent::CoreInfo {
                name: "Snes9x".into(),
                version: "1.63".into(),
                api_version: 1,
            },
            RunnerEvent::AchievementsLoaded {
                ra_game_id: 1234,
                unlocked: 3,
                total: 20,
                hardcore: false,
            },
            RunnerEvent::AchievementUnlocked {
                id: 101,
                hardcore: false,
            },
            RunnerEvent::AchievementsLoginExpired,
        ]
    }

    #[test]
    fn events_round_trip_through_one_line() {
        for event in all_events() {
            let encoded = encode_event_line(&event);
            assert!(encoded.starts_with('\n') && encoded.ends_with('\n'));
            assert_eq!(encoded.trim().lines().count(), 1, "{encoded}");
            assert_eq!(decode_event_line(encoded.trim()), Some(event));
        }
    }

    #[test]
    fn noise_before_an_event_is_skipped() {
        let line = r#"Map_LoROMMap{"type":"Stopped","play_seconds":7}"#;
        assert_eq!(
            decode_event_line(line),
            Some(RunnerEvent::Stopped { play_seconds: 7 })
        );
    }

    #[test]
    fn garbage_and_unknown_events_are_ignored() {
        for line in [
            "",
            "Map_LoROMMap",
            "{\"type\":",
            r#"{"type":"Teleported"}"#,
            r#"{"other":1}"#,
        ] {
            assert_eq!(decode_event_line(line), None, "{line}");
        }
    }

    #[test]
    fn achievement_events_have_stable_wire_names() {
        let line = encode_event_line(&RunnerEvent::AchievementsLoginExpired);
        assert_eq!(line.trim(), r#"{"type":"AchievementsLoginExpired"}"#);
        let unlocked = r#"{"type":"AchievementUnlocked","id":7,"hardcore":true}"#;
        assert_eq!(
            decode_event_line(unlocked),
            Some(RunnerEvent::AchievementUnlocked {
                id: 7,
                hardcore: true
            })
        );
    }

    #[test]
    fn launcher_commands_serialize_with_type_tag() {
        let json = serde_json::to_string(&LauncherCommand::SaveState { slot: 3 }).unwrap();
        assert_eq!(json, r#"{"type":"SaveState","slot":3}"#);
    }
}
