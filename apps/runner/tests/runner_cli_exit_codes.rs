//! End-to-end tests of the runner binary: headless smoke test (determinism,
//! non-black frame), SRAM persistence across runs, and the exit-code contract.
//!
//! Requires the snes9x core fetched by `scripts/fetch-snes9x-core.{sh,ps1}`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::PathBuf;
use std::process::Command;

use support::{core, result_line, runner, scratch, test_rom};

#[test]
fn headless_smoke_is_deterministic_and_not_black() {
    let (a, b) = (scratch("smoke-a"), scratch("smoke-b"));
    let run_a = runner(&core(), &test_rom(), &a, &["--headless-frames", "600"]);
    let run_b = runner(&core(), &test_rom(), &b, &["--headless-frames", "600"]);
    assert_eq!(
        run_a.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run_a.stderr)
    );
    assert_eq!(run_b.status.code(), Some(0));
    let line = result_line(&run_a);
    assert!(
        line.starts_with("frames=600 ") && line.contains(" nonblack=true "),
        "{line}"
    );
    assert_eq!(
        line,
        result_line(&run_b),
        "same ROM + core must give the same final frame"
    );
}

#[test]
fn sram_persists_across_runs() {
    let data = scratch("sram");
    for _ in 0..2 {
        assert_eq!(
            runner(&core(), &test_rom(), &data, &["--headless-frames", "5"])
                .status
                .code(),
            Some(0)
        );
    }
    // Each game has its own folder: ROM name + content id.
    let dir = support::game_saves_dir(&data, "snes", &test_rom());
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("nostyra-test [") && name.ends_with(']'),
        "{name}"
    );
    let srm = &dir.join("save.srm");
    let first = std::fs::read(srm).unwrap()[0];
    runner(&core(), &test_rom(), &data, &["--headless-frames", "5"]);
    // The test ROM increments SRAM byte 0 on every boot.
    assert_eq!(std::fs::read(srm).unwrap()[0], first.wrapping_add(1));

    // A renamed copy of the ROM keeps using the same folder (found by content id).
    let renamed = data.join("Renamed Game.sfc");
    std::fs::copy(test_rom(), &renamed).unwrap();
    runner(&core(), &renamed, &data, &["--headless-frames", "5"]);
    assert_eq!(std::fs::read(srm).unwrap()[0], first.wrapping_add(2));
    assert_eq!(
        std::fs::read_dir(data.join("saves/snes")).unwrap().count(),
        1
    );
}

#[test]
fn legacy_save_folder_is_renamed_on_first_run() {
    let data = scratch("sram-legacy");
    let legacy = data.join("saves/snes/nostyra-test");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("save.srm"), [41u8; 8192]).unwrap();
    runner(&core(), &test_rom(), &data, &["--headless-frames", "5"]);
    let dir = support::game_saves_dir(&data, "snes", &test_rom());
    assert!(!legacy.exists() && dir != legacy);
    assert_eq!(std::fs::read(dir.join("save.srm")).unwrap()[0], 42);
}

#[test]
fn bad_roms_exit_4() {
    let data = scratch("bad-rom");
    let tiny = data.join("tiny.sfc");
    std::fs::write(&tiny, [0xFFu8; 100]).unwrap();
    let huge = data.join("huge.sfc");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(17 * 1024 * 1024)
        .unwrap();
    // `.nes` without an iNES header and no `--system`: no system → ROM rejected.
    let bare = data.join("bare.nes");
    std::fs::write(&bare, vec![0u8; 40 * 1024]).unwrap();
    for rom in [tiny, huge, data.join("missing.sfc"), bare] {
        let out = runner(&core(), &rom, &data, &["--headless-frames", "1"]);
        assert_eq!(out.status.code(), Some(4), "{}", rom.display());
    }
}

/// A real shared library that is not a libretro core (missing `retro_*`).
fn non_libretro_library() -> &'static str {
    if cfg!(windows) {
        "C:/Windows/System32/kernel32.dll"
    } else if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "/lib/x86_64-linux-gnu/libc.so.6"
    }
}

#[test]
fn bad_cores_exit_3() {
    let data = scratch("bad-core");
    let not_a_library = data.join("fake-core.bin");
    std::fs::write(&not_a_library, b"not a shared library").unwrap();
    for core in [not_a_library, PathBuf::from(non_libretro_library())] {
        let out = runner(&core, &test_rom(), &data, &["--headless-frames", "1"]);
        assert_eq!(out.status.code(), Some(3), "{}", core.display());
    }
}

#[test]
fn bad_arguments_exit_2() {
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .arg("--rom")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
