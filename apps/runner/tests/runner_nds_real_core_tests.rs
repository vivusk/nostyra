//! Nintendo DS on melonDS DS through the runner binary (`--system nds` as the
//! launcher passes it): the retail-like test ROM B passes the encrypted-dump
//! pre-check, boots and its EEPROM boot counter persists across runs in
//! `saves/nds/<game> [<ID>]/save.srm`; an encrypted dump (ROM B with a wrong first
//! ARM9 word) is refused before the core with Nostyra's own text (exit 4); a config
//! error the core reports only in its log (DSi console mode without a NAND) is exit
//! 4 with the core's reason in the detail. The runner sets
//! `MELONDSDS_SKIP_ERROR_SCREEN` itself (through msvcrt on Windows). Skipped (with a
//! note) without the core (`scripts/fetch-melondsds-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use support::{game_saves_dir, registry_core, repo, result_line, runner, scratch};
use system_catalog::NDS;

const ROM_B: &str = "test-assets/roms/nostyra-test-save.nds";

fn melonds_ds() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(NDS.id).unwrap();
    registry_core(core.id, "fetch-melondsds-core")
}

fn run(core: &Path, rom: &Path, data: &Path, extra: &[&str]) -> (Output, String) {
    let args = [&["--system", NDS.id, "--headless-frames", "30"][..], extra].concat();
    let out = runner(core, rom, data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (out, stderr)
}

#[test]
fn rom_b_eeprom_boot_counter_persists_in_the_game_save_folder() {
    let Some(core) = melonds_ds() else { return };
    let rom = repo(ROM_B);
    let data = scratch("nds-eeprom");
    let dir = game_saves_dir(&data, NDS.id, &rom);
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.ends_with(']'), "saves/nds/<game> [<ID>]: {name}");
    assert!(dir.starts_with(data.join("saves").join(NDS.id)));
    let srm = dir.join(ipc_protocol::SRAM_FILE_NAME);
    for boot in [1, 2] {
        let (out, stderr) = run(&core, &rom, &data, &[]);
        assert_eq!(out.status.code(), Some(0), "boot {boot}: {stderr}");
        assert!(result_line(&out).contains(" nonblack=true "));
        let save = std::fs::read(&srm).unwrap();
        assert_eq!(save.len(), 8 * 1024, "8 KiB EEPROM");
        assert_eq!(save[..2], [boot, 0xA5], "boot counter + marker");
    }
}

#[test]
fn encrypted_dump_is_refused_before_the_core() {
    let Some(core) = melonds_ds() else { return };
    let data = scratch("nds-encrypted");
    let mut rom = std::fs::read(repo(ROM_B)).unwrap();
    rom[0x4000..0x4004].copy_from_slice(&0x1234_5678_u32.to_le_bytes());
    let encrypted = data.join("encrypted.nds");
    std::fs::write(&encrypted, rom).unwrap();
    let (out, stderr) = run(&core, &encrypted, &data, &[]);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    assert!(stderr.contains("This DS dump is encrypted"), "{stderr}");
}

/// melonDS DS refuses DSi mode without a NAND image: `retro_load_game` fails with no
/// `SET_MESSAGE` (the error screen is skipped), only an error log line, followed by
/// the "Skipping error screen…" notice, which the detail must not show instead
/// (the core's own log lines also reach stderr: match the runner's error line).
#[test]
fn config_error_is_exit_4_with_the_core_reason() {
    let Some(core) = melonds_ds() else { return };
    let data = scratch("nds-dsi-mode");
    let dsi = ["--core-option", "melonds_console_mode=dsi"];
    let (out, stderr) = run(&core, &repo(ROM_B), &data, &dsi);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    let detail = "core rejected the game (retro_load_game returned false): \
                  DSi mode requires a NAND image, but none was found.";
    assert!(stderr.contains(detail), "{stderr}");
}
