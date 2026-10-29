//! Dolphin (Nintendo GameCube + Wii): OpenGL core 3.3 hardware rendering at 1×
//! native (640×528), single-core CPU so every callback stays on the session thread,
//! HLE IPL + HLE IOS (no BIOS), system files from the buildbot `Dolphin.zip`, saves =
//! the core's own `User/` tree in the game's save folder (M14 phase-00 spike on
//! buildbot DLL `2609.0.158+3aac3f052f`).

use crate::core_specs::{CoreAssets, CoreSpec, SaveStateQuality, CORE_DEFAULTS};

/// `RETRO_DEVICE_JOYPAD` (GC pad on GameCube; Wii Remote + Nunchuk on Wii through
/// `dolphin_wiimote_default_device`).
const JOYPAD: u32 = 1;
/// `RETRO_DEVICE_NONE`.
const NONE: u32 = 0;

/// No firmware, no BIOS switch, no memory cards (the core keeps GCI folders and the
/// Wii NAND under `User/`). States: size 0 before frame 1, then ~95–111 MiB
/// (re-queried, 256 MiB cap); serialize steps the emulation → `Serialized`; the core
/// accepts any blob → strict version guard. A refused GL request silently gives a
/// black Null renderer → `hw_render_required`. Unloading after an unserialize hangs
/// the session thread's exit → `keep_library_loaded`.
pub const DOLPHIN: CoreSpec = CoreSpec {
    id: "dolphin-emu",
    library_stem: "dolphin_libretro",
    display_name: "Dolphin",
    systems: &["gc", "wii"],
    license: "GPL-2.0-or-later",
    fixed_options: DOLPHIN_OPTIONS,
    savestates: SaveStateQuality::Serialized,
    port_devices: &[JOYPAD, JOYPAD, NONE, NONE],
    requery_state_size: true,
    max_state_bytes: 256 * 1024 * 1024,
    system_assets: Some(CoreAssets {
        dir: "dolphin-emu",
        markers: &["Sys/codehandler.bin", "Sys/GC/font_western.bin"],
        archive_path: "assets/system/Dolphin.zip",
    }),
    hw_render_required: true,
    strict_state_version: true,
    keep_library_loaded: true,
    ..CORE_DEFAULTS
};

/// Dolphin options (prefix `dolphin_`, 30 of 112 keys; unanswered keys take their
/// declared default, `dolphin_cpu_core` included = the per-arch JIT). Nostyra's
/// choices: single core, hardware renderer (`Null` only via a test `--core-option`),
/// EFB 1× (2× readback p95 7 ms > the 4 ms rule), synchronous shaders (no shared
/// contexts on Windows), no OSD / cheats (no `.cht` beside `system/`), English, HLE
/// IPL, L3 stays a game input (sideways toggle off), per-field audio push, warning
/// log level, Wii Remote + Nunchuk with the IR pointer read from the mouse (mode 2),
/// no rumble / Bluetooth / DSU / network / microphones / custom textures / mods / VR.
pub(crate) const DOLPHIN_OPTIONS: &[(&str, &str)] = &[
    ("dolphin_main_cpu_thread", "disabled"),
    ("dolphin_fastmem", "enabled"),
    ("dolphin_fastmem_arena", "enabled"),
    ("dolphin_cheats_enabled", "disabled"),
    ("dolphin_skip_gc_bios", "enabled"),
    ("dolphin_language", "1"),
    ("dolphin_osd_enabled", "disabled"),
    ("dolphin_log_level", "3"),
    ("dolphin_dsp_hle", "enabled"),
    ("dolphin_call_back_audio_method", "1"),
    ("dolphin_gc_sp1", "255"),
    ("dolphin_enable_gamecube_mic", "disabled"),
    ("dolphin_widescreen", "enabled"),
    ("dolphin_pal60", "enabled"),
    ("dolphin_enable_rumble", "disabled"),
    ("dolphin_wiimote_continuous_scanning", "disabled"),
    ("dolphin_bluetooth_passthrough", "disabled"),
    ("dolphin_wiispeak_enable", "disabled"),
    ("dolphin_renderer", "Hardware"),
    ("dolphin_widescreen_hack", "disabled"),
    ("dolphin_efb_scale", "1"),
    ("dolphin_shader_compilation_mode", "0"),
    ("dolphin_mods_enabled", "disabled"),
    ("dolphin_vr_mode", "disabled"),
    ("dolphin_load_custom_textures", "disabled"),
    ("dolphin_wiimote_default_device", "nunchuk"),
    ("dolphin_hotkey_sideways_toggle", "Disabled"),
    ("dolphin_ir_mode", "2"),
    ("dolphin_cheats_import", "disabled"),
    ("dolphin_dsu_enabled", "disabled"),
];
