//! Shared helpers for runner binary tests: repo paths, the snes9x core, scratch
//! dirs, running the runner and reading its headless result line.
#![allow(dead_code)] // each test crate uses a different subset
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

pub fn core() -> PathBuf {
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

/// `cores/<library stem>` of a registry core, or `None` (with a skip note) when its
/// fetch script has not been run.
pub fn registry_core(core_id: &str, fetch_script: &str) -> Option<PathBuf> {
    let core = system_catalog::core(core_id).expect("registry core");
    let path = repo(&format!(
        "cores/{}.{}",
        core.library_stem,
        std::env::consts::DLL_EXTENSION
    ));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: run scripts/{fetch_script} first");
        None
    }
}

pub fn test_rom() -> PathBuf {
    repo("test-assets/roms/nostyra-test.sfc")
}

/// Fresh per-test scratch dir under cargo's target tmp.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("runner-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn runner(core: &Path, rom: &Path, data: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(rom)
        .arg("--data-dir")
        .arg(data)
        .args(extra)
        .output()
        .unwrap()
}

/// Runs `command` like `output()`, but fails the test after `deadline` instead of
/// hanging: stdout / stderr are drained on threads (never blocking the wait), the
/// child is polled, and on timeout it is killed and the test panics with `what`.
pub fn output_within(mut command: Command, deadline: std::time::Duration, what: &str) -> Output {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() >= deadline {
            let _ = child.kill();
            panic!("{what}: still running after {deadline:?} (killed)");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    Output {
        status,
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    }
}

/// The `frames=… checksum=… nonblack=… loaded=…` line (cores may print other stdout noise).
pub fn result_line(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find(|l| l.starts_with("frames="))
        .expect("result line")
        .to_string()
}

/// The save folder the runner uses for `rom` when no `--content-sha1` is passed
/// (it hashes the ROM like the scanner).
pub fn game_saves_dir(data: &Path, system: &str, rom: &Path) -> PathBuf {
    let spec = system_catalog::system(system).expect("registry system");
    let sha1 = game_library::game_content_sha1(rom, spec).unwrap();
    ipc_protocol::saves_dir_for_game(data, system, rom, &sha1)
}

/// The game config layer file the runner reads for `rom` (named like its save folder).
pub fn game_config_path(data: &Path, system: &str, rom: &Path) -> PathBuf {
    let spec = system_catalog::system(system).expect("registry system");
    let sha1 = game_library::game_content_sha1(rom, spec).unwrap();
    ipc_protocol::game_config_path_for_game(data, system, rom, &sha1)
}

/// True when OpenGL tests can run here (`hw_render_gl::probe_gl_context`). Without an
/// OpenGL 3.3 context (CI without a GPU, macOS test threads) they skip with a note,
/// unless `NOSTYRA_REQUIRE_GL=1` turns the skip into a failure. `NOSTYRA_FORCE_NO_GL=1`
/// (the runner's debug hook) skips them too, as on a machine without OpenGL.
pub fn gl_test_gate() -> bool {
    let required = std::env::var("NOSTYRA_REQUIRE_GL").is_ok_and(|v| v == "1");
    if std::env::var("NOSTYRA_FORCE_NO_GL").is_ok_and(|v| v == "1") {
        assert!(!required, "NOSTYRA_REQUIRE_GL=1 with NOSTYRA_FORCE_NO_GL=1");
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL=1");
        return false;
    }
    if cfg!(target_os = "macos") {
        assert!(!required, "GL tests cannot run on macOS test threads");
        eprintln!("skipped: SDL windows need the main thread on macOS");
        return false;
    }
    match hw_render_gl::probe_gl_context() {
        Ok(()) => true,
        Err(error) if required => panic!("NOSTYRA_REQUIRE_GL=1 but no OpenGL 3.3: {error}"),
        Err(error) => {
            eprintln!("skipped: no OpenGL 3.3 context ({error})");
            false
        }
    }
}
