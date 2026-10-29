//! Command-line interface. Invalid arguments exit with code 2 (clap default),
//! matching the runner's exit-code contract for the M1 launcher.

use std::path::PathBuf;

use clap::Parser;
use config_layers::{is_valid_shader, BUNDLED_PRESETS, MAX_RUN_AHEAD_FRAMES};
use ipc_protocol::StateSlotId;
/// Highest save-state slot number (slots 0..=MAX_STATE_SLOT), shared with the launcher.
pub use ipc_protocol::MAX_STATE_SLOT;

use self::state_slot_arg::parse_slot_id;

mod state_slot_arg;

/// Plays one ROM with one libretro core.
#[derive(Debug, Parser)]
#[command(name = "nostyra-runner", version, about)]
pub struct RunnerCliArgs {
    /// Path to the libretro core library (e.g. snes9x_libretro.dll).
    #[arg(long, required_unless_present = "probe_core")]
    pub core: Option<PathBuf>,

    /// Path to the game ROM.
    #[arg(long, required_unless_present = "probe_core")]
    pub rom: Option<PathBuf>,

    /// System of the ROM (`snes`, `nes`): saves folder, system config layer and
    /// size limits. The launcher always passes it; without it the ROM is classified
    /// by extension + content like the library scanner.
    #[arg(long, value_name = "ID")]
    pub system: Option<String>,

    /// Content sha1 of the game (`games.sha1`): the id in its save folder and game
    /// config names. The launcher always passes it; without it the runner hashes
    /// the ROM like the library scanner (discs: first data track).
    #[arg(long, value_name = "HEX", value_parser = parse_content_sha1)]
    pub content_sha1: Option<String>,

    /// Launcher mode: JSON-line events on stdout and a pending play-session file
    /// under `<data>/sessions/pending/` when a started session ends.
    #[arg(long)]
    pub ipc: bool,

    /// Check that a file is a loadable libretro core, print its `CoreInfo` event on
    /// stdout and exit (0 ok, 3 not a core). Used by the launcher after a download.
    #[arg(long, value_name = "CORE", conflicts_with_all = ["core", "rom"])]
    pub probe_core: Option<PathBuf>,

    /// Start in borderless fullscreen (F11 toggles).
    #[arg(long)]
    pub fullscreen: bool,

    /// Force audio-paced timing instead of auto-selecting by display refresh.
    #[arg(long)]
    pub audio_sync: bool,

    /// Save-state slot used by F1 (save) / F4 (load).
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=i64::from(MAX_STATE_SLOT)))]
    pub state_slot: u8,

    /// Load this slot (0-9, or `auto` = the resume point written on exit) at start and
    /// make it current (launcher "Load" / "Continue"); a missing or incompatible slot
    /// shows an error and the game starts normally.
    #[arg(long, value_name = "N|auto", conflicts_with = "state_slot", value_parser = parse_slot_id)]
    pub load_state: Option<StateSlotId>,

    /// Override the data directory (saves/, states/, system/); used by tests.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,

    /// Test-only: run N frames headless (no window/audio/input), print the final
    /// frame checksum, persist SRAM, exit.
    #[arg(long, hide = true)]
    pub headless_frames: Option<u32>,

    /// Test-only: after the last headless frame, save state slot N or `auto` (state,
    /// sidecar and thumbnail, like F1 / auto-save on exit).
    #[arg(long, hide = true, value_name = "N|auto", requires = "headless_frames", value_parser = parse_slot_id)]
    pub headless_save_slot: Option<StateSlotId>,

    /// Print the effective config (config layers + CLI overrides) as JSON on
    /// stdout and exit, after the core and ROM load and before any window opens.
    #[arg(long)]
    pub print_config: bool,

    /// Override `video.shader` from the config layers (testing / power users).
    #[arg(long, hide = true, value_parser = parse_shader)]
    pub shader: Option<String>,

    /// Override `latency.run_ahead_frames` (0 = off).
    #[arg(long, hide = true, value_parser = clap::value_parser!(u8).range(0..=i64::from(MAX_RUN_AHEAD_FRAMES)))]
    pub run_ahead: Option<u8>,

    /// Override `rewind.enabled` (`on` / `off`).
    #[arg(long, hide = true, value_parser = parse_on_off)]
    pub rewind: Option<bool>,

    /// Test-only: answer this core option (`KEY=VALUE`, repeatable); overrides the
    /// registry's fixed options of the core.
    #[arg(long = "core-option", hide = true, value_name = "KEY=VALUE", value_parser = crate::runner_core_options::parse_core_option)]
    pub core_options: Vec<(String, String)>,

    /// Folder holding the bundled slang presets (`crt/<name>.slangp`); the launcher
    /// passes its resource folder. Else `NOSTYRA_SHADERS`, then `shaders/` beside the exe.
    #[arg(long, hide = true, value_name = "DIR")]
    pub shaders_dir: Option<PathBuf>,
}

impl RunnerCliArgs {
    /// Numbered slot active at start (F1/F4, slot grid); `--load-state auto` keeps
    /// `--state-slot`.
    pub fn start_slot(&self) -> u8 {
        state_slot_arg::start_slot(self.load_state, self.state_slot)
    }
}

/// At least the hex digits a folder name keeps (`ipc_protocol::content_id`).
fn parse_content_sha1(value: &str) -> Result<String, String> {
    ipc_protocol::content_id(value)
        .map(|_| value.to_string())
        .ok_or_else(|| "expected a hex sha1".to_string())
}

/// Same rule as the `video.shader` config key.
fn parse_shader(value: &str) -> Result<String, String> {
    if is_valid_shader(value) {
        Ok(value.to_string())
    } else {
        Err(format!(
            "expected `none`, one of {BUNDLED_PRESETS:?}, or an absolute .slangp path"
        ))
    }
}

fn parse_on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("expected `on` or `off`".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_command_line() {
        let args = RunnerCliArgs::try_parse_from([
            "nostyra-runner",
            "--core",
            "c.dll",
            "--rom",
            "g.sfc",
            "--fullscreen",
            "--state-slot",
            "3",
        ])
        .unwrap();
        assert!(args.fullscreen && !args.audio_sync && !args.ipc);
        assert_eq!(args.state_slot, 3);
        assert_eq!(args.headless_frames, None);
        assert_eq!(args.rom, Some(PathBuf::from("g.sfc")));
    }

    #[test]
    fn config_overrides_parse_and_validate() {
        let base = ["nostyra-runner", "--core", "c", "--rom", "r"];
        let parse = |extra: &[&str]| RunnerCliArgs::try_parse_from(base.iter().chain(extra));
        let args = parse(&[
            "--print-config",
            "--shader",
            "crt-geom",
            "--run-ahead",
            "2",
            "--rewind",
            "on",
            "--shaders-dir",
            "res/shaders",
        ])
        .unwrap();
        assert!(args.print_config);
        assert_eq!(args.shaders_dir, Some(PathBuf::from("res/shaders")));
        assert_eq!(args.shader.as_deref(), Some("crt-geom"));
        assert_eq!((args.run_ahead, args.rewind), (Some(2), Some(true)));
        let defaults = parse(&[]).unwrap();
        assert_eq!(
            (defaults.shader, defaults.run_ahead, defaults.rewind),
            (None, None, None)
        );
        assert!(parse(&["--run-ahead", "3"]).is_err());
        assert!(parse(&["--shader", "crt-royale"]).is_err());
        assert!(parse(&["--rewind", "yes"]).is_err());
    }

    #[test]
    fn probe_core_needs_no_rom_and_excludes_game_args() {
        let probe = RunnerCliArgs::try_parse_from(["nostyra-runner", "--probe-core", "c.dll"]);
        assert_eq!(probe.unwrap().probe_core, Some(PathBuf::from("c.dll")));
        let mixed = ["nostyra-runner", "--probe-core", "c.dll", "--rom", "g.sfc"];
        assert!(RunnerCliArgs::try_parse_from(mixed).is_err());
    }

    #[test]
    fn rejects_missing_rom_and_bad_slot() {
        assert!(RunnerCliArgs::try_parse_from(["nostyra-runner", "--core", "c.dll"]).is_err());
        let bad_slot = [
            "nostyra-runner",
            "--core",
            "c",
            "--rom",
            "r",
            "--state-slot",
            "10",
        ];
        assert!(RunnerCliArgs::try_parse_from(bad_slot).is_err());
    }
}
