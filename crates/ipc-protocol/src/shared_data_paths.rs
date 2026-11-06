//! Data locations every app must agree on (desktop launcher, runner, web edition).
//! Pure path builders under a given data root; the OS default roots and the per-game
//! entry lookup on disk live in `native_data_roots` (feature `native`).
//!
//! Per-game layout (names from `game_folder_key`: `<rom key> [<content id>]`):
//! saves in `<data>/saves/<system>/<name>/`, game config layer in
//! `<data>/config/game/<system>/<name>.toml`.

use std::path::{Path, PathBuf};

/// Marker file shipped beside the exes in the portable zip: its presence makes the copy
/// keep everything (data, data-folder choice) in its own folder.
pub const PORTABLE_MARKER_FILE: &str = "portable.txt";
/// Data root folder name inside a portable copy.
pub const PORTABLE_DATA_DIR: &str = "data";
/// The desktop launcher's library database, `<data>/nostyra.db` (the web edition
/// keeps its own database in the browser and never opens this one).
pub const DATABASE_FILE_NAME: &str = "nostyra.db";

/// `<data>/sessions/pending/`: one JSON file per finished session, written by the
/// runner (`--ipc`) or the web edition, imported and removed by the launcher.
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

/// Extension of a game config layer file.
pub const GAME_CONFIG_EXT: &str = ".toml";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_under_the_data_root() {
        let root = Path::new("data");
        assert_eq!(
            saves_system_dir(root, "nes"),
            root.join("saves").join("nes")
        );
        assert_eq!(
            game_config_system_dir(root, "nes"),
            root.join("config").join("game").join("nes")
        );
        assert_eq!(system_dir(root), root.join("system"));
        assert_eq!(
            staged_content_dir(root),
            root.join("temp").join("staged-content")
        );
        assert_eq!(
            pending_sessions_dir(root),
            root.join("sessions").join("pending")
        );
    }
}
