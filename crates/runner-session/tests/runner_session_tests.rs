//! Real runner, spawned through runner-session: events, exit classification,
//! launch timeout, pending play-time import into storage-db, core probe.
//!
//! Requires `cargo build -p nostyra-runner` (done by `cargo test --workspace`), the
//! snes9x core (`scripts/fetch-snes9x-core.*`) and the repo test ROM.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::time::Duration;

use runner_session::{
    import_pending_sessions, probe_core_with_runner, spawn_runner_session, RunnerLaunch,
    SessionErrorKind, SessionOutcome, SessionUpdate, DEFAULT_LAUNCH_TIMEOUT,
};
use storage_db::{
    add_library_folder, find_game_id_by_rom_path, game_play_stats, open_in_memory_database,
    upsert_scanned_batch, Connection, ScannedGame,
};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// `NOSTYRA_RUNNER`, else `target/<profile>/nostyra-runner` next to this test's `deps/`.
fn runner_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("NOSTYRA_RUNNER") {
        return path.into();
    }
    let exe = std::env::current_exe().unwrap();
    let profile_dir = exe.parent().and_then(Path::parent).unwrap();
    let path = profile_dir.join(format!("nostyra-runner{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.exists(),
        "missing {}: run `cargo build -p nostyra-runner` first",
        path.display()
    );
    path
}

fn core() -> PathBuf {
    let path = repo(&format!(
        "cores/snes9x_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    assert!(
        path.exists(),
        "missing {}; run scripts/fetch-snes9x-core first",
        path.display()
    );
    path
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("session-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn launch(rom: &Path, data: &Path) -> RunnerLaunch {
    RunnerLaunch {
        runner: runner_binary(),
        core: core(),
        rom: rom.to_path_buf(),
        system: "snes".into(),
        data_dir: data.to_path_buf(),
        extra_args: vec!["--headless-frames".into(), "30".into()],
        env: Vec::new(),
    }
}

/// All updates until `Finished` (the channel closes right after it).
fn run_to_end(launch: &RunnerLaunch, timeout: Duration) -> (Vec<SessionUpdate>, SessionOutcome) {
    let session = spawn_runner_session(launch, timeout).unwrap();
    let updates: Vec<_> = session.updates.iter().collect();
    match updates.last() {
        Some(SessionUpdate::Finished(outcome)) => (updates.clone(), outcome.clone()),
        other => panic!("no Finished update: {other:?}"),
    }
}

/// Library DB containing `rom` (path string exactly as the launcher passes it).
fn library_with(rom: &Path) -> Connection {
    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "roms", 1).unwrap();
    let game = ScannedGame {
        platform_id: "snes".into(),
        folder_id: folder.id,
        rom_path: rom.to_string_lossy().into_owned(),
        file_size: 32_768,
        file_mtime_ns: 0,
        crc32: "0".into(),
        sha1: "0".into(),
        title: "Nostyra Test".into(),
        region: None,
        dat_name: None,
        metadata: None,
        serial: None,
    };
    upsert_scanned_batch(&mut conn, &[game], 1).unwrap();
    conn
}

#[test]
fn successful_session_is_reported_and_play_time_imported_once() {
    let data = scratch("ok");
    let rom = repo("test-assets/roms/nostyra-test.sfc");
    let (updates, outcome) = run_to_end(&launch(&rom, &data), DEFAULT_LAUNCH_TIMEOUT);
    assert!(
        matches!(&updates[0], SessionUpdate::Started { core, .. } if core == "Snes9x"),
        "{updates:?}"
    );
    assert_eq!((outcome.exit_code, outcome.error.clone()), (Some(0), None));
    assert!(outcome.started_at.is_some());

    let conn = library_with(&rom);
    let pending = ipc_protocol::pending_sessions_dir(&data);
    let file = std::fs::read_dir(&pending)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let backup = std::fs::read(&file).unwrap();
    let first = import_pending_sessions(&conn, &data).unwrap();
    assert_eq!(first.imported, 1);
    assert_eq!(
        std::fs::read_dir(&pending).unwrap().count(),
        0,
        "file removed"
    );

    // A file left behind by an interrupted import must not count twice.
    std::fs::write(&file, backup).unwrap();
    let second = import_pending_sessions(&conn, &data).unwrap();
    assert_eq!((second.imported, second.duplicates), (0, 1));
    let game = find_game_id_by_rom_path(&conn, &rom.to_string_lossy())
        .unwrap()
        .unwrap();
    assert_eq!(game_play_stats(&conn, game).unwrap().session_count, 1);
}

#[test]
fn bad_rom_is_a_rom_error_without_session() {
    let data = scratch("bad-rom");
    let tiny = data.join("tiny.sfc");
    std::fs::write(&tiny, [0u8; 100]).unwrap();
    let (updates, outcome) = run_to_end(&launch(&tiny, &data), DEFAULT_LAUNCH_TIMEOUT);
    assert_eq!(updates.len(), 1, "no Started: {updates:?}");
    let error = outcome.error.unwrap();
    assert_eq!((error.code, error.kind), (Some(4), SessionErrorKind::Rom));
    assert!(error.detail.contains("too small"), "{}", error.detail);
    assert_eq!(outcome.started_at, None);
    let conn = library_with(&tiny);
    assert_eq!(import_pending_sessions(&conn, &data).unwrap().imported, 0);
}

#[test]
fn launch_timeout_is_reported_before_start() {
    let data = scratch("timeout");
    let rom = repo("test-assets/roms/nostyra-test.sfc");
    let (updates, outcome) = run_to_end(&launch(&rom, &data), Duration::ZERO);
    assert_eq!(updates[0], SessionUpdate::LaunchTimedOut, "{updates:?}");
    assert!(updates
        .iter()
        .any(|u| matches!(u, SessionUpdate::Started { .. })));
    assert_eq!(outcome.exit_code, Some(0));
}

#[test]
fn probe_through_runner() {
    let probed = probe_core_with_runner(&runner_binary(), &core()).unwrap();
    assert_eq!(probed.name, "Snes9x");
    let fake = scratch("probe").join("fake.dll");
    std::fs::write(&fake, b"nope").unwrap();
    let err = probe_core_with_runner(&runner_binary(), &fake).unwrap_err();
    assert!(err.contains("not a usable libretro core"), "{err}");
}

#[test]
fn runner_args_pass_the_game_system() {
    let launch = RunnerLaunch {
        system: "nes".into(),
        ..launch(Path::new("roms/game.nes"), Path::new("data"))
    };
    let args: Vec<String> = runner_session::runner_args(&launch)
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let at = args.iter().position(|a| a == "--system").expect("--system");
    assert_eq!(args[at + 1], "nes");
    assert_eq!(args.last().map(String::as_str), Some("30"), "extra last");
}
