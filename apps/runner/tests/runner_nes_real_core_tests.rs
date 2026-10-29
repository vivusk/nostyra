//! NES end to end through the runner binary on Mesen with `nostyra-test.nes`:
//! battery SRAM persists across runs (boot counter) under `saves/nes/`, and a slot
//! saved headless loads back before frame 1 (`--system nes` as the launcher passes it).
//!
//! Skips with a note when the mesen core is missing (`scripts/fetch-mesen-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{repo, result_line, runner, scratch};

fn mesen() -> Option<PathBuf> {
    let core = system_catalog::core("mesen").unwrap();
    let path = repo(&format!(
        "cores/{}.{}",
        core.library_stem,
        std::env::consts::DLL_EXTENSION
    ));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: run scripts/fetch-mesen-core first");
        None
    }
}

fn nes_rom() -> PathBuf {
    repo("test-assets/roms/nostyra-test.nes")
}

fn run_ok(core: &Path, data: &Path, extra: &[&str]) -> String {
    let mut args = vec!["--system", "nes"];
    args.extend_from_slice(extra);
    let out = runner(core, &nes_rom(), data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    result_line(&out)
}

#[test]
fn nes_sram_persists_across_runs() {
    let Some(core) = mesen() else { return };
    let data = scratch("nes-sram");
    let srm = support::game_saves_dir(&data, "nes", &nes_rom()).join(ipc_protocol::SRAM_FILE_NAME);
    run_ok(&core, &data, &["--headless-frames", "30"]);
    let first = std::fs::read(&srm).unwrap();
    assert_eq!(&first[..2], &[1, 0xA5], "boot counter 1 + marker");
    run_ok(&core, &data, &["--headless-frames", "30"]);
    assert_eq!(
        std::fs::read(&srm).unwrap()[0],
        2,
        "counter survived the restart"
    );
}

#[test]
fn nes_saved_slot_loads_before_first_frame() {
    let Some(core) = mesen() else { return };
    let data = scratch("nes-slot");
    let saved = run_ok(
        &core,
        &data,
        &["--headless-frames", "90", "--headless-save-slot", "1"],
    );
    assert!(saved.ends_with("loaded=false"), "{saved}");
    let slots = support::game_saves_dir(&data, "nes", &nes_rom());
    assert!(slots.join("slot1.state").metadata().unwrap().len() > 0);
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    let warm1 = run_ok(
        &core,
        &data,
        &["--headless-frames", "5", "--load-state", "1"],
    );
    let warm2 = run_ok(
        &core,
        &data,
        &["--headless-frames", "5", "--load-state", "1"],
    );
    assert!(warm1.ends_with("loaded=true"), "{warm1}");
    assert_eq!(
        checksum(&warm1),
        checksum(&warm2),
        "same state, same frames"
    );
    let cold = run_ok(&core, &data, &["--headless-frames", "5"]);
    assert_ne!(
        checksum(&cold),
        checksum(&warm1),
        "the slot was really loaded"
    );
}
