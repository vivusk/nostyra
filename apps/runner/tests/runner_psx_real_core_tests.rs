//! PlayStation end to end through the runner binary on PCSX ReARMed with the
//! self-written test discs (`--system psx` as the launcher passes it, HLE BIOS forced
//! with the hidden `--core-option`): `.cue` and `.m3u` games run, memory card 1
//! (`save.srm`, 128 KiB) survives a run unchanged with no `.bak`, card 2 is created in
//! the game's save folder, a slot saved headless loads before frame 1, and a cue with
//! a missing track fails with the invalid-ROM code and the scanner's reason.
//!
//! Skips with a note when the core is missing (`scripts/fetch-pcsx-rearmed-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{repo, result_line, runner, scratch};

const HLE: [&str; 2] = ["--core-option", "pcsx_rearmed_bios=HLE"];

fn pcsx() -> Option<PathBuf> {
    let core = system_catalog::PCSX_REARMED;
    let path = repo(&format!(
        "cores/{}.{}",
        core.library_stem,
        std::env::consts::DLL_EXTENSION
    ));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: run scripts/fetch-pcsx-rearmed-core first");
        None
    }
}

fn disc(file: &str) -> PathBuf {
    repo(&format!("test-assets/roms/{file}"))
}

fn saves(data: &Path, game: &Path) -> PathBuf {
    support::game_saves_dir(data, system_catalog::PSX.id, game)
}

fn run(core: &Path, game: &Path, data: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["--system", system_catalog::PSX.id];
    args.extend_from_slice(&HLE);
    args.extend_from_slice(extra);
    runner(core, game, data, &args)
}

fn run_ok(core: &Path, game: &Path, data: &Path, extra: &[&str]) -> String {
    let out = run(core, game, data, extra);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{}: {stderr}", game.display());
    result_line(&out)
}

/// A formatted 128 KiB card ("MC" header frame with its XOR checksum) with a marker
/// in the last block, which the test program never touches.
fn seeded_card() -> Vec<u8> {
    let mut card = vec![0u8; 131_072];
    card[..2].copy_from_slice(b"MC");
    card[127] = card[..127].iter().fold(0, |x, b| x ^ b);
    card[0x1E000..0x1E010].copy_from_slice(b"NOSTYRA-MARKER-1");
    card
}

#[test]
fn cue_and_m3u_games_run_and_memory_cards_persist() {
    let Some(core) = pcsx() else { return };
    for file in ["nostyra-test-psx.cue", "nostyra-test-psx.m3u"] {
        let data = scratch(&format!("psx-cards-{file}"));
        let game = disc(file);
        let dir = saves(&data, &game);
        std::fs::create_dir_all(&dir).unwrap();
        let srm = dir.join(ipc_protocol::SRAM_FILE_NAME);
        std::fs::write(&srm, seeded_card()).unwrap();
        let line = run_ok(&core, &game, &data, &["--headless-frames", "120"]);
        assert!(line.contains("nonblack=true"), "{file}: {line}");
        assert_eq!(
            std::fs::read(&srm).unwrap(),
            seeded_card(),
            "{file}: card 1 kept"
        );
        let mut backup = srm.clone().into_os_string();
        backup.push(".bak");
        assert!(!Path::new(&backup).exists(), "{file}: no .srm.bak");
        assert!(
            dir.join("pcsx-card2.mcd").is_file(),
            "{file}: card 2 in the game dir"
        );
    }
}

#[test]
fn psx_saved_slot_loads_before_first_frame() {
    let Some(core) = pcsx() else { return };
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    let data = scratch("psx-slot");
    let game = disc("nostyra-test-psx.cue");
    let saved = run_ok(
        &core,
        &game,
        &data,
        &["--headless-frames", "90", "--headless-save-slot", "1"],
    );
    assert!(saved.ends_with("loaded=false"), "{saved}");
    let load = ["--headless-frames", "5", "--load-state", "1"];
    let warm1 = run_ok(&core, &game, &data, &load);
    let warm2 = run_ok(&core, &game, &data, &load);
    assert!(warm1.ends_with("loaded=true"), "{warm1}");
    assert_eq!(checksum(&warm1), checksum(&warm2), "same frames");
    let cold = run_ok(&core, &game, &data, &["--headless-frames", "5"]);
    assert_ne!(checksum(&cold), checksum(&warm1), "slot loaded");
}

#[test]
fn cue_with_a_missing_track_fails_with_the_scan_reason() {
    let Some(core) = pcsx() else { return };
    let data = scratch("psx-broken");
    let cue = data.join("Broken.cue");
    std::fs::write(&cue, "FILE \"Gone.bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();
    let out = run(&core, &cue, &data, &["--headless-frames", "5"]);
    assert_eq!(out.status.code(), Some(ipc_protocol::EXIT_ROM_LOAD));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("track file 'Gone.bin' not found — fix the FILE line or rename the file"),
        "{stderr}"
    );
}
