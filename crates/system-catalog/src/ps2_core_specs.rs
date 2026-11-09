//! PCEE2 (Sony PlayStation 2): the PCSX2 libretro core (upstream PCSX2 v2.9,
//! `library_name` "PCEE2"). M18 phase 00 spike on buildbot DLL v2.9.138; LRPS2
//! ("ParaLLEl PS2") was spiked beside it and is not shipped (GL 3.3 shader failures,
//! shader dumps into the cwd, off-thread `GET_VARIABLE`, title-screen stall).

use crate::core_specs::{CoreSpec, FirmwareRule, SaveStateQuality, CORE_DEFAULTS};

/// `RETRO_DEVICE_JOYPAD` (DualShock 2 on ports 1–2) / `RETRO_DEVICE_NONE`.
const JOYPAD: u32 = 1;
const NONE: u32 = 0;

/// Folder under `<data>/system/` PCSX2 scans for BIOS dumps (spike step 4: the
/// core also writes `<stem>.nvm` / `.mec` beside the dump, so it must be writable).
pub const PS2_BIOS_RULE: FirmwareRule = FirmwareRule::Ps2Romdir {
    subdir: "pcsx2/bios",
    option: "pcsx2_bios",
};

/// Folder under `<data>/system/` where PCEE2 keeps the shared memory cards
/// (`Mcd001.ps2` / `Mcd002.ps2`, all PS2 games): cloud sync treats it as one unit.
pub const PS2_MEMCARD_SUBDIR: &str = "pcsx2/memcards";

/// No firmware list (the ROMDIR rule validates any dump), no memory-card slots in
/// the save folder (shared `Mcd001.ps2` / `Mcd002.ps2` under `system/pcsx2/memcards`,
/// as on a real console). States: a constant 96 MiB from before frame 1 (no requery,
/// no boot hold; `exact_state_size` safe), serialize steps the VM → `Serialized`;
/// half-length / middle-flipped blobs are accepted by the core → host length + CRC
/// guard, strict version guard (states are core-specific). A refused GL request
/// falls back to the core's own readback device with a correct picture, so
/// `hw_render_required` stays false. Pads on ports 1–2 only (no multitap).
/// Library kept loaded: after `retro_deinit` the core still owns a thread / window
/// callback inside the DLL, so `FreeLibrary` ends in a `pcee2_libretro.dll_unloaded`
/// crash (0xC000041D, ~8 s "not responding" first); the runner hard-exits instead.
pub const PCEE2: CoreSpec = CoreSpec {
    id: "pcee2",
    library_stem: "pcee2_libretro",
    display_name: "PCEE2",
    systems: &["ps2"],
    license: "GPL-3.0-or-later",
    fixed_options: PCEE2_OPTIONS,
    firmware_rule: PS2_BIOS_RULE,
    savestates: SaveStateQuality::Serialized,
    port_devices: &[JOYPAD, JOYPAD, NONE, NONE],
    exact_state_size: true,
    max_state_bytes: 128 * 1024 * 1024,
    strict_state_version: true,
    hardcore_required_options: PCEE2_HARDCORE,
    keep_library_loaded: true,
    ..CORE_DEFAULTS
};

/// PCEE2 options (prefix `pcsx2_`, 14 of 46 keys; every unanswered key takes its
/// declared default; `pcsx2_bios` is answered per launch by the runner's picker).
/// Nostyra's choices (spike steps 3, 5, 6): OpenGL on the host's context at 1×
/// (2× readback p95 30 ms ≫ the 4 ms rule), one field per `retro_run` (the default
/// `skip_duplicate_frames` blocks until a unique frame = 2 fields, doubling audio),
/// the host paces (the internal limiter doubles `run_frame`), fast boot, no cheats /
/// over- or underclock / widescreen or no-interlacing patches (hardcore rules), no
/// rumble (the host has no rumble interface), no multitap / lightgun / USB mouse.
pub(crate) const PCEE2_OPTIONS: &[(&str, &str)] = &[
    ("pcsx2_renderer", "opengl"),
    ("pcsx2_upscale_multiplier", "1"),
    ("pcsx2_skip_duplicate_frames", "disabled"),
    ("pcsx2_frame_limiter", "frontend"),
    ("pcsx2_fast_boot", "enabled"),
    ("pcsx2_cheats", "disabled"),
    ("pcsx2_ee_cycle_rate", "0"),
    ("pcsx2_ee_cycle_skip", "0"),
    ("pcsx2_widescreen_patches", "disabled"),
    ("pcsx2_no_interlacing_patches", "disabled"),
    ("pcsx2_rumble", "disabled"),
    ("pcsx2_multitap", "disabled"),
    ("pcsx2_lightgun", "disabled"),
    ("pcsx2_usb_mouse", "disabled"),
];

/// RetroAchievements hardcore rules for PCEE2. rcheevos' own disallowed-option
/// table has no PCSX2 / PCEE2 row, so this list is the only enforcer: no cheats, EE
/// cycle rate 100 % (`0`), cycle skipping off (`0`), no widescreen / no-interlacing
/// patches. The strings equal the fixed options above (pinned by a test).
pub(crate) const PCEE2_HARDCORE: &[(&str, &str)] = &[
    ("pcsx2_cheats", "disabled"),
    ("pcsx2_ee_cycle_rate", "0"),
    ("pcsx2_ee_cycle_skip", "0"),
    ("pcsx2_widescreen_patches", "disabled"),
    ("pcsx2_no_interlacing_patches", "disabled"),
];

#[cfg(test)]
#[path = "ps2_core_spec_tests.rs"]
mod tests;
