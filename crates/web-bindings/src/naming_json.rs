//! Names shared with the desktop: the per-game entry `<rom key> [ID]` (save folder
//! and game config layer), its lookup in a folder listing, web save-state stems.

use std::path::Path;

use ipc_protocol::{
    content_id, game_entry_name, pick_game_entry_name, web_state_stem, StateSlotId,
};

/// `<rom key> [ID]<ext>` for a new entry of the ROM file `rom_name`; `None` when
/// `sha1` is not a usable content sha1.
pub fn new_entry_name(rom_name: &str, sha1: &str, ext: &str) -> Option<String> {
    let id = content_id(sha1)?;
    Some(game_entry_name(Path::new(rom_name), &id, ext))
}

/// The existing entry among `names` (one folder listing) for `sha1`, if any.
pub fn find_entry_name(names: &[String], sha1: &str, ext: &str) -> Option<String> {
    let id = content_id(sha1)?;
    pick_game_entry_name(names.iter().map(String::as_str), &id, ext).map(str::to_owned)
}

/// `web-slot<n>` (`slot` 0..=9) or `web-auto` (`slot` < 0); `None` past the last slot.
pub fn state_stem(slot: i32) -> Option<String> {
    let id = if slot < 0 {
        StateSlotId::Auto
    } else {
        StateSlotId::Numbered(
            u8::try_from(slot)
                .ok()
                .filter(|s| *s <= ipc_protocol::MAX_STATE_SLOT)?,
        )
    };
    Some(web_state_stem(id))
}

/// EmulatorJS build the web states come from (sidecar `core_version`): an upgrade
/// changes it, so an old web state is never loaded silently.
pub const WEB_CORE_VERSION: &str = "emulatorjs-4.2.3";

/// Sidecar JSON of a web save state (`build = "web"`), same format as the desktop's.
/// `rom_patch_crc32` = CRC of the soft-patch chain found beside the ROM (the resume
/// point's `patch-changed` warning compares it, as on the desktop).
pub fn web_state_sidecar(
    ejs_core: &str,
    created_at: u64,
    sram_crc32: Option<u32>,
    rom_patch_crc32: Option<u32>,
    state: &[u8],
) -> String {
    let sidecar = ipc_protocol::StateSidecar {
        core_name: ejs_core.to_string(),
        core_version: WEB_CORE_VERSION.to_string(),
        created_at,
        sram_crc32,
        rom_patch_crc32,
        state_len: Some(state.len() as u64),
        state_crc32: Some(crc32(state)),
        build: Some("web".to_string()),
        ..Default::default()
    };
    serde_json::to_string(&sidecar).unwrap_or_default()
}

/// CRC32 (IEEE) as the desktop sidecars use.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_sidecar_is_a_desktop_sidecar_with_build_web() {
        let json = web_state_sidecar("snes9x", 5, Some(7), Some(9), b"abc");
        let parsed: ipc_protocol::StateSidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.build.as_deref(), Some("web"));
        assert_eq!(parsed.core_version, WEB_CORE_VERSION);
        assert_eq!(parsed.state_len, Some(3));
        assert_eq!(parsed.rom_patch_crc32, Some(9));
        assert_eq!(parsed.state_crc32, Some(0x3524_41C2), "crc32(\"abc\")");
    }

    const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

    #[test]
    fn entry_names_match_the_desktop() {
        assert_eq!(
            new_entry_name("Zelda: Link?.sfc", SHA1, "").as_deref(),
            Some("Zelda_ Link_ [6B47BB75]")
        );
        assert_eq!(
            new_entry_name("Game.sfc", SHA1, ".toml").as_deref(),
            Some("Game [6B47BB75].toml")
        );
        assert_eq!(new_entry_name("Game.sfc", "xyz", ""), None);
        let names = vec![
            "Other [00000000]".to_string(),
            "Renamed [6B47BB75]".to_string(),
        ];
        assert_eq!(
            find_entry_name(&names, SHA1, "").as_deref(),
            Some("Renamed [6B47BB75]")
        );
    }

    #[test]
    fn web_state_stems() {
        assert_eq!(state_stem(0).as_deref(), Some("web-slot0"));
        assert_eq!(state_stem(-1).as_deref(), Some("web-auto"));
        assert_eq!(state_stem(10), None);
    }
}
