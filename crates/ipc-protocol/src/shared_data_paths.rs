//! Data locations both processes must agree on, and the pending play-session file
//! the runner leaves for the launcher.
//!
//! Per-game layout (names from `game_folder_key`: `<rom key> [<content id>]`):
//! saves in `<data>/saves/<system>/<name>/`, game config layer in
//! `<data>/config/game/<system>/<name>.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::game_folder_key::resolve_game_entry;

/// Default data root: `<exe dir>/data` in a portable copy ([`portable_dir`]), else per
/// user (`%APPDATA%\Nostyra\data` on Windows). Runner and launcher both call this, so
/// their defaults cannot drift; `None` if the OS has no home directory.
pub fn default_data_root() -> Option<PathBuf> {
    if let Some(dir) = portable_dir() {
        return Some(dir.join(PORTABLE_DATA_DIR));
    }
    directories::ProjectDirs::from("", "", "Nostyra").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Marker file shipped beside the exes in the portable zip: its presence makes the copy
/// keep everything (data, data-folder choice) in its own folder.
pub const PORTABLE_MARKER_FILE: &str = "portable.txt";
/// Data root folder name inside a portable copy.
pub const PORTABLE_DATA_DIR: &str = "data";

/// The folder of the running exe when it holds [`PORTABLE_MARKER_FILE`] (portable
/// copy); `None` for an installed or dev build. Launcher and runner sit side by side,
/// so both see the same marker.
pub fn portable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    portable_dir_of(exe.parent()?)
}

/// [`portable_dir`] for a given exe folder (pure, for tests).
pub fn portable_dir_of(exe_dir: &Path) -> Option<PathBuf> {
    exe_dir
        .join(PORTABLE_MARKER_FILE)
        .is_file()
        .then(|| exe_dir.to_path_buf())
}

/// `<data>/sessions/pending/`: one JSON file per finished session, written by the
/// runner (`--ipc`), imported and removed by the launcher.
pub fn pending_sessions_dir(data_root: &Path) -> PathBuf {
    data_root.join("sessions").join("pending")
}

/// `<data>/config/`: root of the TOML config layers (global/system/core/game).
pub fn config_dir(data_root: &Path) -> PathBuf {
    data_root.join("config")
}

/// `<data>/saves/`: every game's save folder (the launcher serves thumbnails from it).
pub fn saves_root(data_root: &Path) -> PathBuf {
    data_root.join("saves")
}

/// `<data>/system/`: BIOS and other core system files (the core's
/// `GET_SYSTEM_DIRECTORY`; the launcher's Settings → BIOS adds files here).
pub fn system_dir(data_root: &Path) -> PathBuf {
    data_root.join("system")
}

/// `<data>/temp/staged-content/`: one `<pid>/` folder per running runner holding the
/// content file staged for a core that loads by path (soft-patched or long-path
/// cartridges); removed on exit, stale folders cleaned at the next runner start.
pub fn staged_content_dir(data_root: &Path) -> PathBuf {
    data_root.join("temp").join("staged-content")
}

/// Battery SRAM file in the game's folder: the runner reads/writes it, the launcher
/// compares its CRC with a resume point's `sram_crc32`.
pub const SRAM_FILE_NAME: &str = "save.srm";

/// Cartridge clock file beside the SRAM, for cores that keep the clock apart from it
/// (`RETRO_MEMORY_RTC`, e.g. mGBA's Game Boy MBC3 games); written by the runner only.
pub const RTC_FILE_NAME: &str = "rtc.bin";

/// `<data>/saves/<system>/`: the save folders of one system.
pub fn saves_system_dir(data_root: &Path, system: &str) -> PathBuf {
    saves_root(data_root).join(system)
}

/// `<data>/config/game/<system>/`: the game config layers of one system.
pub fn game_config_system_dir(data_root: &Path, system: &str) -> PathBuf {
    config_dir(data_root).join("game").join(system)
}

/// The game's own folder: SRAM, save-state slots and the core's save directory.
/// `content_sha1` is the game's stored sha1 (`games.sha1`); see `resolve_game_entry`
/// (may rename a legacy folder; never creates one).
pub fn saves_dir_for_game(
    data_root: &Path,
    system: &str,
    rom_path: &Path,
    content_sha1: &str,
) -> PathBuf {
    resolve_game_entry(
        &saves_system_dir(data_root, system),
        rom_path,
        content_sha1,
        "",
    )
}

/// The game config layer file. The only place this path is built: config-layers
/// and the launcher both take it from here.
pub fn game_config_path_for_game(
    data_root: &Path,
    system: &str,
    rom_path: &Path,
    content_sha1: &str,
) -> PathBuf {
    resolve_game_entry(
        &game_config_system_dir(data_root, system),
        rom_path,
        content_sha1,
        GAME_CONFIG_EXT,
    )
}

/// Extension of a game config layer file.
pub const GAME_CONFIG_EXT: &str = ".toml";

/// Content of a pending session file. Survives the launcher closing mid-game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSessionRecord {
    /// Exactly the `--rom` argument, which the launcher takes from `games.rom_path`.
    pub rom_path: String,
    /// Unix seconds when the session started.
    pub started_at: i64,
    pub play_seconds: u64,
    pub exit_code: i32,
}

/// Unique, sortable file name: `<started_ms>-<pid>.json`.
pub fn pending_session_file_name(started_unix_ms: u128, pid: u32) -> String {
    format!("{started_unix_ms}-{pid}.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_ends_in_nostyra_data_dir() {
        let root = default_data_root().unwrap();
        assert!(
            root.to_string_lossy().contains("Nostyra"),
            "{}",
            root.display()
        );
    }

    #[test]
    fn portable_only_with_the_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(portable_dir_of(dir.path()), None);
        std::fs::write(dir.path().join(PORTABLE_MARKER_FILE), "").unwrap();
        assert_eq!(portable_dir_of(dir.path()), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn per_game_paths_share_the_name() {
        let root = Path::new("no-such-data-dir");
        let rom = Path::new("E:/ROMs/Zelda: Link?.sfc");
        let sha1 = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";
        assert_eq!(
            saves_dir_for_game(root, "snes", rom, sha1),
            root.join("saves")
                .join("snes")
                .join("Zelda_ Link_ [6B47BB75]")
        );
        assert_eq!(
            game_config_path_for_game(root, "snes", rom, sha1),
            root.join("config")
                .join("game")
                .join("snes")
                .join("Zelda_ Link_ [6B47BB75].toml")
        );
    }

    #[test]
    fn pending_layout_and_record_json() {
        let dir = pending_sessions_dir(Path::new("data"));
        assert_eq!(dir, Path::new("data").join("sessions").join("pending"));
        assert_eq!(
            pending_session_file_name(1_700_000_000_123, 42),
            "1700000000123-42.json"
        );
        let record = PendingSessionRecord {
            rom_path: "E:/ROMs/a.sfc".into(),
            started_at: 1_700_000_000,
            play_seconds: 90,
            exit_code: 0,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<PendingSessionRecord>(&json).unwrap(),
            record
        );
    }
}
