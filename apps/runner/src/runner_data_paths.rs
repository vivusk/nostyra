//! Where the runner keeps per-game data: one folder per game, named after the
//! ROM plus its content id, so users can find (and back up) everything for a game
//! in one place and renaming or moving a ROM keeps its saves.
//!
//! Layout: `<data>/saves/<system>/<rom key> [<content id>]/` holding `save.srm`,
//! `slotN.*` / `auto.*` state files; `<data>/system/` (BIOS / core system files).
//! The names and folder layout come from `ipc_protocol` so the launcher computes
//! the same paths.

use std::path::{Path, PathBuf};

use anyhow::Context;
use system_catalog::SystemSpec;

#[derive(Clone, Debug)]
pub struct RunnerDataPaths {
    /// `--data-dir` or the OS per-user data root (config layers live below it).
    pub data_root: PathBuf,
    /// Where `--ipc` sessions leave their play-time record for the launcher
    /// (created on write, only when a session started).
    pub pending_sessions_dir: PathBuf,
    pub system_dir: PathBuf,
    /// Core's save directory (`GET_SAVE_DIRECTORY`): the game's own folder.
    pub saves_dir: PathBuf,
    pub sram_file: PathBuf,
    /// Cartridge clock (`rtc.bin`) beside the SRAM; only games with a core RTC use it.
    pub rtc_file: PathBuf,
    /// Same folder as `saves_dir`; state slots live next to the SRAM.
    pub states_dir: PathBuf,
    /// `<data>/temp/staged-content/<pid>/`: content staged for a core that loads by
    /// path (created only when used, removed with the session).
    pub staging_dir: PathBuf,
    /// File-name-safe ROM name (window title, readable part of the folder name).
    pub rom_name: String,
    /// The game's content sha1 (`--content-sha1` or hashed): save folder and game
    /// config layer id.
    pub content_sha1: String,
    /// The game's system: saves subfolder and `config/system/<id>.toml` layer.
    pub system: &'static SystemSpec,
    /// Disc / revision the session's states are tagged with (GameCube / Wii only).
    pub state_content_tag: Option<String>,
}

impl RunnerDataPaths {
    /// Resolves paths under `data_dir` (or the OS per-user data dir) and creates them.
    /// `content_sha1`: the launcher's `--content-sha1`; `None` hashes the ROM.
    pub fn resolve(
        data_dir: Option<&Path>,
        rom: &Path,
        system: &'static SystemSpec,
        content_sha1: Option<&str>,
    ) -> anyhow::Result<Self> {
        // Same root as the launcher (chosen folder, else default): both call
        // ipc_protocol::data_root.
        let root = match data_dir {
            Some(dir) => dir.to_path_buf(),
            None => {
                ipc_protocol::data_root().context("cannot determine the user data directory")?
            }
        };
        let content_sha1 = match content_sha1 {
            Some(sha1) => sha1.to_string(),
            None => game_library::game_content_sha1(rom, system)
                .map_err(|err| anyhow::anyhow!("cannot read {}: {err}", rom.display()))?,
        };
        let game_dir = ipc_protocol::saves_dir_for_game(&root, system.id, rom, &content_sha1);
        let paths = Self {
            pending_sessions_dir: ipc_protocol::pending_sessions_dir(&root),
            system_dir: ipc_protocol::system_dir(&root),
            sram_file: game_dir.join(ipc_protocol::SRAM_FILE_NAME),
            rtc_file: game_dir.join(ipc_protocol::RTC_FILE_NAME),
            states_dir: game_dir.clone(),
            staging_dir: ipc_protocol::staged_content_dir(&root)
                .join(std::process::id().to_string()),
            saves_dir: game_dir,
            rom_name: ipc_protocol::rom_key(rom),
            content_sha1,
            data_root: root,
            system,
            state_content_tag: game_library::state_content_tag(system, rom),
        };
        for dir in [&paths.system_dir, &paths.saves_dir] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

    #[test]
    fn resolve_creates_layout_under_override() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-paths-")
            .tempdir()
            .unwrap();
        let root = temp.path();
        let paths = RunnerDataPaths::resolve(
            Some(root),
            Path::new("Game.nes"),
            &system_catalog::NES,
            Some(SHA1),
        )
        .unwrap();
        assert!(paths.states_dir.is_dir() && paths.saves_dir.is_dir());
        let game_dir = root.join("saves").join("nes").join("Game [6B47BB75]");
        assert_eq!(paths.sram_file, game_dir.join(ipc_protocol::SRAM_FILE_NAME));
        assert_eq!(paths.rtc_file, game_dir.join(ipc_protocol::RTC_FILE_NAME));
        assert_eq!(paths.states_dir, game_dir);
        assert_eq!(paths.data_root, root);
    }

    #[test]
    fn folder_and_title_use_the_shared_rom_key() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-paths-key-")
            .tempdir()
            .unwrap();
        let root = temp.path();
        let paths = RunnerDataPaths::resolve(
            Some(root),
            Path::new("roms/CON.sfc"),
            &system_catalog::SNES,
            Some(SHA1),
        )
        .unwrap();
        assert_eq!(paths.rom_name, "CON_");
        assert_eq!(
            paths.saves_dir,
            root.join("saves").join("snes").join("CON_ [6B47BB75]")
        );
    }

    #[test]
    fn without_a_sha1_the_rom_is_hashed() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-paths-hash-")
            .tempdir()
            .unwrap();
        let rom = temp.path().join("Game.sfc");
        std::fs::write(&rom, [0u8; 1024]).unwrap();
        let paths =
            RunnerDataPaths::resolve(Some(temp.path()), &rom, &system_catalog::SNES, None).unwrap();
        // sha1 of 1 KiB of zeros.
        assert_eq!(
            paths.content_sha1,
            "60CACBF3D72E1E7834203DA608037B1BF83B40E8"
        );
        assert!(paths.saves_dir.ends_with("Game [60CACBF3]"));
    }
}
