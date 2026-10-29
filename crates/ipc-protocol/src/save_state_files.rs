//! Save-state files in a game's save folder (`saves_dir_for_game`): the runner
//! writes them (F1, pause menu, auto-save on exit), the launcher lists, deletes and
//! loads them (`--load-state N|auto`). Per slot ([`StateSlotId`]): `<stem>.state`
//! (raw core data), `<stem>.json` ([`StateSidecar`]) and an optional `<stem>.png`
//! thumbnail, where the stem is `slotN` or `auto` (the resume point).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state_slot_id::StateSlotId;

/// Highest save-state slot number (slots `0..=MAX_STATE_SLOT`).
pub const MAX_STATE_SLOT: u8 = 9;

/// Raw `retro_serialize` data; its presence means the slot is used.
pub const STATE_FILE_EXT: &str = "state";
/// [`StateSidecar`] JSON.
pub const SIDECAR_FILE_EXT: &str = "json";
/// Thumbnail PNG, written after the state (may be missing).
pub const THUMBNAIL_FILE_EXT: &str = "png";

/// Every file a slot can own, for deletion.
pub const STATE_SLOT_FILE_EXTS: [&str; 3] = [STATE_FILE_EXT, SIDECAR_FILE_EXT, THUMBNAIL_FILE_EXT];

/// `<saves dir>/<slotN|auto>.<ext>`: the only place state file names are built.
pub fn state_file(saves_dir: &Path, id: StateSlotId, ext: &str) -> PathBuf {
    saves_dir.join(format!("{}.{ext}", id.file_stem()))
}

/// `<saves dir>/slot<slot>.<ext>` (numbered slots; see [`state_file`]).
pub fn state_slot_file(saves_dir: &Path, slot: u8, ext: &str) -> PathBuf {
    state_file(saves_dir, StateSlotId::Numbered(slot), ext)
}

/// `<stem>.json`: which core wrote the state and when, plus what the launcher needs to
/// warn before resuming (`auto`) and what the runner checks before handing the state
/// to the core. The optional fields are absent in older sidecars (no check then).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSidecar {
    pub core_name: String,
    pub core_version: String,
    /// Unix seconds.
    pub created_at: u64,
    /// CRC32 of the core's SRAM when the state was saved (= the bytes the exit flush
    /// writes to `save.srm`); `None` when the game has no SRAM. A different `.srm`
    /// CRC later means in-game progress newer than the state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sram_crc32: Option<u32>,
    /// CRC32 of the soft-patch file found next to the ROM, applied or not; `None` =
    /// no patch file. A different CRC later means the patch changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom_patch_crc32: Option<u32>,
    /// Length of the `.state` file (M14): a different length = damaged state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_len: Option<u64>,
    /// CRC32 of the `.state` file (M14): a different CRC = damaged state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_crc32: Option<u32>,
    /// Which disc / revision the state was made from, for games sharing one save
    /// folder across discs and revisions (GameCube / Wii: `ID6:disc:rev`, `m3u:ID6`;
    /// see `game_library::state_content_tag`). `None` = no check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_tag: Option<String>,
    /// RetroAchievements runtime progress (hex of `rc_client_serialize_progress`) when
    /// an achievement session was active (M15); `None` = restore resets progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub achievements_progress: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_file_names() {
        let dir = Path::new("saves");
        assert_eq!(
            state_slot_file(dir, 3, STATE_FILE_EXT),
            dir.join("slot3.state")
        );
        assert_eq!(
            STATE_SLOT_FILE_EXTS.map(|ext| state_slot_file(dir, 0, ext)),
            [
                dir.join("slot0.state"),
                dir.join("slot0.json"),
                dir.join("slot0.png")
            ]
        );
        assert_eq!(
            STATE_SLOT_FILE_EXTS.map(|ext| state_file(dir, StateSlotId::Auto, ext)),
            [
                dir.join("auto.state"),
                dir.join("auto.json"),
                dir.join("auto.png")
            ]
        );
    }

    #[test]
    fn old_sidecar_parses_and_new_fields_are_written_only_when_set() {
        let old = br#"{"core_name":"Snes9x","core_version":"1.62","created_at":5}"#;
        let sidecar: StateSidecar = serde_json::from_slice(old).unwrap();
        assert_eq!((sidecar.sram_crc32, sidecar.rom_patch_crc32), (None, None));
        assert_eq!(
            (
                sidecar.state_len,
                sidecar.state_crc32,
                sidecar.content_tag.clone()
            ),
            (None, None, None)
        );
        let json = serde_json::to_string(&sidecar).unwrap();
        assert!(
            !json.contains("crc32") && !json.contains("state_len"),
            "{json}"
        );
        assert!(!json.contains("content_tag"), "{json}");
        assert!(!json.contains("achievements_progress"), "{json}");

        let full = StateSidecar {
            sram_crc32: Some(7),
            rom_patch_crc32: Some(9),
            state_len: Some(4),
            state_crc32: Some(11),
            content_tag: Some("GALE01:0:0".into()),
            achievements_progress: Some("00ff".into()),
            ..sidecar
        };
        let json = serde_json::to_string(&full).unwrap();
        assert!(json.contains("\"sram_crc32\":7") && json.contains("\"rom_patch_crc32\":9"));
        assert!(json.contains("\"state_len\":4") && json.contains("\"state_crc32\":11"));
        assert!(json.contains("\"content_tag\":\"GALE01:0:0\""));
        assert!(json.contains("\"achievements_progress\":\"00ff\""));
        assert_eq!(serde_json::from_str::<StateSidecar>(&json).unwrap(), full);
    }
}
