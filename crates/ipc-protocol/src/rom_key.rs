//! `<rom name>` part of the per-game save folder and game config names (the readable
//! part; `game_folder_key` adds the content id that actually finds them) and the
//! runner's window title. Launcher and runner both derive it here.

use std::path::Path;

/// Keeps names well inside path-length limits (Windows MAX_PATH).
const MAX_NAME_CHARS: usize = 120;
/// Windows device names that can't be used as file names (any extension).
const RESERVED_WINDOWS_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// ROM file stem as a safe single file-name component. Keeps Unicode (e.g.
/// Vietnamese/Japanese titles); replaces characters Windows forbids and control
/// chars with `_`; strips leading dots and trailing dots/spaces (no `..`
/// traversal, no hidden files); suffixes reserved Windows device names.
pub fn rom_key(rom_path: &Path) -> String {
    let raw = rom_path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(MAX_NAME_CHARS)
        .collect();
    let name = cleaned
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .trim_start();
    if name.is_empty() {
        return "game".to_string();
    }
    let device = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED_WINDOWS_NAMES
        .iter()
        .any(|r| r.eq_ignore_ascii_case(device))
    {
        // Windows reserves `COM1` *and* `COM1.anything`, so suffix the device part.
        return format!("{device}_{}", &name[device.len()..]);
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(path: &str) -> String {
        rom_key(Path::new(path))
    }

    #[test]
    fn keeps_titles_readable_including_unicode() {
        assert_eq!(
            name("roms/Super Game (USA) [!].sfc"),
            "Super Game (USA) [!]"
        );
        assert_eq!(name("Pokémon.sfc"), "Pokémon");
        assert_eq!(name("ドラゴンクエスト V.smc"), "ドラゴンクエスト V");
        assert_eq!(
            name("Chrono Trigger.sfc"),
            name("D:/other folder/Chrono Trigger.sfc")
        );
    }

    #[test]
    fn neutralises_forbidden_chars_and_traversal() {
        assert_eq!(name("Zelda: Link?.sfc"), "Zelda_ Link_");
        assert_eq!(name("../../evil.sfc"), "evil");
        assert_eq!(name("a/..sfc"), "game"); // stem "."
        assert_eq!(name("..."), "game");
        assert_eq!(name(".hidden.sfc"), "hidden");
        assert_eq!(name("trailing. .sfc"), "trailing");
    }

    #[test]
    fn reserved_windows_names_are_suffixed() {
        assert_eq!(name("CON.sfc"), "CON_");
        assert_eq!(name("nul.sfc"), "nul_");
        assert_eq!(name("COM1.v2.sfc"), "COM1_.v2");
        assert_eq!(name("CONTRA.sfc"), "CONTRA");
    }

    #[test]
    fn long_names_are_capped() {
        let long = format!("{}.sfc", "x".repeat(300));
        assert_eq!(name(&long).chars().count(), MAX_NAME_CHARS);
    }
}
