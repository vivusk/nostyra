//! CLI parsing tests (game args, overrides, probe mode).

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
