//! Boot wait, picture, RAM counter, reset and core-file checks for the PS2 test
//! program (submodule of `ps2_real_core_suite_tests`). The debug block layout
//! matches `test-assets/roms/src/ps2_test_elf.py`: `'NSTY'` at +0, the frame counter
//! at +4, read through `RETRO_MEMORY_SYSTEM_RAM` (PCEE2: EE RAM from offset 0).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use libretro_host::{CoreSession, NullAudioSink};
use system_catalog::{firmware_dir, PCEE2};
use video_renderer::RgbaFrameBuffer;

use crate::dreamcast_real_core_checks_tests::pixel;
use crate::runner_held_input::HeldInput;

/// EE RAM address of the program's debug block.
pub(crate) const DEBUG_BLOCK: usize = 0x0010_8000;
/// `'NSTY'` as the EE stores it (little-endian word).
pub(crate) const MAGIC: u32 = 0x5954_534E;
/// Longest the kernel's fast boot may take to start the ELF (spike: ~14 frames).
const BOOT_FRAMES: usize = 600;

/// Runs `frames` frames with nothing pressed; returns the last picture.
pub(crate) fn run(session: &mut CoreSession, frames: usize) -> RgbaFrameBuffer {
    let mut video = RgbaFrameBuffer::default();
    let (mut audio, mut input) = (NullAudioSink::default(), HeldInput::None);
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    video
}

/// Little-endian word at `offset` of the debug block (0 before the core exposes RAM).
pub(crate) fn debug_word(session: &mut CoreSession, offset: usize) -> u32 {
    let at = DEBUG_BLOCK + offset;
    session.system_ram().map_or(0, |ram| {
        u32::from_le_bytes(ram[at..at + 4].try_into().unwrap())
    })
}

/// The program's frame counter (one step per VSINT).
pub(crate) fn counter(session: &mut CoreSession) -> u32 {
    debug_word(session, 4)
}

/// Runs frames until the program's main loop counts (magic present, counter ≥ 4);
/// returns the frames it took. Panics after [`BOOT_FRAMES`].
pub(crate) fn boot(session: &mut CoreSession) -> usize {
    let mut frames = 0;
    while debug_word(session, 0) != MAGIC || counter(session) < 4 {
        assert!(
            frames < BOOT_FRAMES,
            "ELF main loop within {BOOT_FRAMES} frames"
        );
        run(session, 1);
        frames += 1;
    }
    frames
}

fn centre(video: &RgbaFrameBuffer) -> [u8; 3] {
    pixel(video, 0.5, 0.5)
}

/// The sprite fills the screen with a non-black colour that ramps per frame: the
/// centre is lit, and frame N and frame N+30 differ.
pub(crate) fn check_picture(session: &mut CoreSession) {
    let video = run(session, 2);
    let (width, height) = video.size();
    assert!(width >= 512 && height >= 224, "picture {width}x{height}");
    let first = centre(&video);
    assert!(first.iter().any(|&c| c > 16), "non-black centre: {first:?}");
    let later = centre(&run(session, 30));
    assert_ne!(first, later, "colour changes between frames N and N+30");
}

/// One VSINT per `retro_run` (`pcsx2_skip_duplicate_frames=disabled`: one field).
pub(crate) fn check_counter_advances(session: &mut CoreSession) {
    let before = counter(session);
    run(session, 30);
    let advanced = counter(session) - before;
    assert!(
        (25..=35).contains(&advanced),
        "VSINTs in 30 frames: {advanced}"
    );
}

/// `retro_reset` reboots the console: the BIOS starts the ELF again and the counter
/// restarts below its pre-reset value.
pub(crate) fn check_reset_reboots(session: &mut CoreSession) {
    let before = counter(session);
    assert!(before > 40, "ran a while before the reset: {before}");
    session.reset();
    let mut frames = 0;
    while debug_word(session, 0) != MAGIC || counter(session) >= before || counter(session) < 4 {
        assert!(frames < BOOT_FRAMES, "rebooted within {BOOT_FRAMES} frames");
        run(session, 1);
        frames += 1;
    }
    eprintln!(
        "pcee2 suite: counter {} after the reset ({frames} frames)",
        counter(session)
    );
}

/// Every file below `dir`, relative to it, with `/` separators.
fn files_below(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap();
                found.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found
}

/// The core wrote `.nvm` + `.mec` beside (one of) the copied BIOS dump(s) and both
/// memory cards under `<system>/pcsx2/memcards/`; no card lands in the save folder.
pub(crate) fn check_core_files(system: &Path, saves: &Path, stems: &[String]) {
    let bios = firmware_dir(system, &PCEE2);
    let sidecars = stems.iter().any(|stem| {
        let file = bios.join(stem);
        file.with_extension("nvm").is_file() && file.with_extension("mec").is_file()
    });
    assert!(
        sidecars,
        ".nvm and .mec beside a BIOS in {}",
        bios.display()
    );
    let memcards = bios.parent().unwrap().join("memcards");
    for card in ["Mcd001.ps2", "Mcd002.ps2"] {
        let size = std::fs::metadata(memcards.join(card)).map(|m| m.len()).ok();
        assert!(size.is_some_and(|s| s > 8 << 20), "{card}: {size:?}");
    }
    let cards_in_saves: Vec<String> = files_below(saves)
        .into_iter()
        .filter(|f| f.ends_with(".ps2"))
        .collect();
    assert!(
        cards_in_saves.is_empty(),
        "memory cards in saves/: {cards_in_saves:?}"
    );
}
