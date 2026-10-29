//! Mega Drive end to end through the runner binary on Genesis Plus GX with the
//! self-written test ROM: battery SRAM persists (boot counter) under
//! `saves/genesis/` with a stable 2-byte `.srm` (GPGX trims cart RAM to its last
//! non-0xFF byte) and no `.srm.bak`; a slot saved headless loads before frame 1; an
//! `.smd` copy shares the raw dump's save folder; a `.bin` is classified by content;
//! an IPS patch applies although the core loads by path (staged content); a content
//! path of 256+ bytes plays.
//!
//! Skips with a note when the core is missing (`scripts/fetch-genesis-plus-gx-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{repo, result_line, runner, scratch};

fn gpgx() -> Option<PathBuf> {
    support::registry_core("genesis_plus_gx", "fetch-genesis-plus-gx-core")
}

fn test_rom() -> Vec<u8> {
    std::fs::read(repo("test-assets/roms/nostyra-test.gen")).unwrap()
}

/// Copies the test ROM (or `bytes`) to `dir/name`.
fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn run_ok(core: &Path, rom: &Path, data: &Path, extra: &[&str]) -> String {
    let out = runner(core, rom, data, extra);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{}: {stderr}", rom.display());
    result_line(&out)
}

fn srm(data: &Path, rom: &Path) -> Vec<u8> {
    let dir = support::game_saves_dir(data, "genesis", rom);
    std::fs::read(dir.join(ipc_protocol::SRAM_FILE_NAME)).unwrap()
}

#[test]
fn genesis_sram_persists_with_a_stable_size() {
    let Some(core) = gpgx() else { return };
    let data = scratch("sega-sram");
    let rom = repo("test-assets/roms/nostyra-test.gen");
    let frames = ["--system", "genesis", "--headless-frames", "30"];
    run_ok(&core, &rom, &data, &frames);
    assert_eq!(
        srm(&data, &rom),
        [1, 0xA5],
        "boot counter 1 + marker, trimmed"
    );
    run_ok(&core, &rom, &data, &frames);
    assert_eq!(srm(&data, &rom), [2, 0xA5], "counter survived, size stable");
    let dir = support::game_saves_dir(&data, "genesis", &rom);
    assert!(!dir.join("save.srm.bak").exists(), "no .srm.bak");
}

#[test]
fn genesis_saved_slot_loads_before_first_frame() {
    let Some(core) = gpgx() else { return };
    let data = scratch("sega-slot");
    let rom = repo("test-assets/roms/nostyra-test.gen");
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    let system = ["--system", "genesis"];
    let save = [
        &system[..],
        &["--headless-frames", "90", "--headless-save-slot", "1"],
    ]
    .concat();
    let saved = run_ok(&core, &rom, &data, &save);
    assert!(saved.ends_with("loaded=false"), "{saved}");
    let load = [
        &system[..],
        &["--headless-frames", "5", "--load-state", "1"],
    ]
    .concat();
    let warm1 = run_ok(&core, &rom, &data, &load);
    let warm2 = run_ok(&core, &rom, &data, &load);
    assert!(warm1.ends_with("loaded=true"), "{warm1}");
    assert_eq!(checksum(&warm1), checksum(&warm2), "same frames");
    let cold = run_ok(
        &core,
        &rom,
        &data,
        &[&system[..], &["--headless-frames", "5"]].concat(),
    );
    assert_ne!(checksum(&cold), checksum(&warm1), "slot loaded");
}

#[test]
fn smd_copy_shares_saves_and_bin_is_classified_by_content() {
    let Some(core) = gpgx() else { return };
    let data = scratch("sega-formats");
    let roms = data.join("roms");
    let raw = test_rom();
    let md = put(&roms, "Game.md", &raw);
    let smd = put(&roms, "Game.smd", &system_catalog::raw_to_smd(&raw));
    let bin = put(&roms, "Game.bin", &raw);
    let frames = ["--headless-frames", "30"];
    run_ok(&core, &md, &data, &frames);
    run_ok(&core, &smd, &data, &frames);
    let dir = support::game_saves_dir(&data, "genesis", &md);
    assert_eq!(dir, support::game_saves_dir(&data, "genesis", &smd));
    assert_eq!(srm(&data, &md), [2, 0xA5], ".smd continued the .md counter");
    // No --system: the runner classifies the `.bin` (SEGA at 0x100) as genesis.
    run_ok(&core, &bin, &data, &frames);
    assert_eq!(srm(&data, &bin), [3, 0xA5], "same content id, same folder");
}

/// IPS record changing the SRAM marker immediate (`move.b #$A5,$200001` =
/// `13FC 00A5 0020 0001`) to 0x5A.
fn marker_ips(rom: &[u8]) -> Vec<u8> {
    let code = [0x13, 0xFC, 0x00, 0xA5, 0x00, 0x20, 0x00, 0x01];
    let at = rom.windows(code.len()).position(|w| w == code).unwrap() + 3;
    let mut ips = b"PATCH".to_vec();
    ips.extend_from_slice(&(at as u32).to_be_bytes()[1..]);
    ips.extend_from_slice(&[0, 1, 0x5A]);
    ips.extend_from_slice(b"EOF");
    ips
}

#[test]
fn ips_patch_applies_on_a_path_loading_core() {
    let Some(core) = gpgx() else { return };
    let data = scratch("sega-ips");
    let roms = data.join("roms");
    let raw = test_rom();
    let rom = put(&roms, "Game.md", &raw);
    put(&roms, "Game.ips", &marker_ips(&raw));
    let frames = ["--system", "genesis", "--headless-frames", "30"];
    run_ok(&core, &rom, &data, &frames);
    assert_eq!(srm(&data, &rom)[1], 0x5A, "patched marker");
    let staged = ipc_protocol::staged_content_dir(&data);
    let left = std::fs::read_dir(&staged).map_or(0, |dir| dir.count());
    assert_eq!(left, 0, "staged content removed after the session");
    std::fs::remove_file(roms.join("Game.ips")).unwrap();
    std::fs::remove_file(support::game_saves_dir(&data, "genesis", &rom).join("save.srm")).unwrap();
    run_ok(&core, &rom, &data, &frames);
    assert_eq!(srm(&data, &rom)[1], 0xA5, "unpatched marker");
}

#[test]
fn content_path_of_256_bytes_or_more_plays() {
    let Some(core) = gpgx() else { return };
    let data = scratch("sega-long-path");
    let mut dir = data.join("roms");
    while dir.join("Game.md").to_string_lossy().len() < 300 {
        dir = dir.join("a-long-folder-name");
    }
    let rom = put(&dir, "Game.md", &test_rom());
    assert!(rom.to_string_lossy().len() >= 300);
    run_ok(
        &core,
        &rom,
        &data,
        &["--system", "genesis", "--headless-frames", "30"],
    );
    assert_eq!(srm(&data, &rom), [1, 0xA5], "game ran and saved");
}
