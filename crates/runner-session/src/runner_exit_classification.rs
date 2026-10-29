//! Maps a runner exit status to a user-facing error category.

use ipc_protocol::{
    EXIT_CORE_LOAD, EXIT_GRAPHICS, EXIT_OK, EXIT_ROM_LOAD, EXIT_RUNTIME, EXIT_USAGE,
};
use serde::Serialize;

/// Why a session failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionErrorKind {
    /// Exit 2: the launcher passed bad arguments (a bug, not a user problem).
    Usage,
    /// Exit 3: core missing or not a libretro core.
    Core,
    /// Exit 4: ROM missing, wrong size or rejected by the core.
    Rom,
    /// Exit 5: failure while running (GPU, audio…).
    Runtime,
    /// Exit 6: the core needs a graphics API this PC cannot provide (OpenGL 3.3).
    Graphics,
    /// Any other code, a Windows exception (e.g. `0xC0000005`) or a Unix signal.
    Crashed,
}

/// `None` for a clean exit. `code` is `None` when the process was killed by a signal.
pub fn classify_exit(code: Option<i32>) -> Option<SessionErrorKind> {
    match code {
        Some(EXIT_OK) => None,
        Some(EXIT_USAGE) => Some(SessionErrorKind::Usage),
        Some(EXIT_CORE_LOAD) => Some(SessionErrorKind::Core),
        Some(EXIT_ROM_LOAD) => Some(SessionErrorKind::Rom),
        Some(EXIT_RUNTIME) => Some(SessionErrorKind::Runtime),
        Some(EXIT_GRAPHICS) => Some(SessionErrorKind::Graphics),
        _ => Some(SessionErrorKind::Crashed),
    }
}

impl SessionErrorKind {
    /// Short sentence for the UI; details come from the runner's error message.
    pub fn user_message(self) -> &'static str {
        match self {
            Self::Usage => "Nostyra could not start the game (internal launch error).",
            Self::Core => {
                "The emulator core could not be loaded. Try re-downloading it in Settings."
            }
            Self::Rom => {
                "This game file could not be loaded. It may be damaged or not playable here."
            }
            Self::Runtime => "The game stopped because of an error while running.",
            Self::Graphics => "This system needs OpenGL 3.3 graphics — update the graphics driver.",
            Self::Crashed => "The game crashed.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_table() {
        let table = [
            (Some(0), None),
            (Some(2), Some(SessionErrorKind::Usage)),
            (Some(3), Some(SessionErrorKind::Core)),
            (Some(4), Some(SessionErrorKind::Rom)),
            (Some(5), Some(SessionErrorKind::Runtime)),
            (Some(6), Some(SessionErrorKind::Graphics)),
            (Some(1), Some(SessionErrorKind::Crashed)),
            (Some(101), Some(SessionErrorKind::Crashed)), // Rust panic exit code
            (
                Some(0xC000_0005_u32 as i32),
                Some(SessionErrorKind::Crashed),
            ), // access violation
            (None, Some(SessionErrorKind::Crashed)),      // killed by signal
        ];
        for (code, expected) in table {
            assert_eq!(classify_exit(code), expected, "{code:?}");
        }
    }

    #[test]
    fn graphics_kind_serialises_and_explains() {
        assert_eq!(
            serde_json::to_string(&SessionErrorKind::Graphics).unwrap(),
            "\"graphics\""
        );
        assert!(SessionErrorKind::Graphics
            .user_message()
            .contains("OpenGL 3.3"));
    }

    #[test]
    fn rom_message_names_no_system() {
        let message = SessionErrorKind::Rom.user_message();
        assert!(message.contains("could not be loaded"));
        assert!(!message.contains("SNES"), "{message}");
        assert!(!message.contains("  "), "{message}");
    }
}
