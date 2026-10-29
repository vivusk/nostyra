//! Flycast (Sega Dreamcast): OpenGL hardware rendering (GL core 3.2 request, served
//! by the host's 3.3 core context), HLE BIOS unless a real `dc_boot.bin` is present,
//! VMUs as files in the game's save folder (M11 phase-00 spike).

use crate::core_specs::{BiosOptionSwitch, CoreSpec, SaveStateQuality, CORE_DEFAULTS};
use crate::firmware_lists::FLYCAST_BIOS;

/// `RETRO_DEVICE_JOYPAD` / `RETRO_DEVICE_NONE` (libretro.h).
const JOYPAD: u32 = 1;
const NONE: u32 = 0;

/// An invalid `dc/dc_boot.bin` would still be booted (fatal SH-4 exception), so the
/// runner answers `reicast_hle_bios` from the file's md5 (`bios_switch`).
/// Flycast connects no controller and no VMU until all four ports are set: pads on
/// ports 1–2, nothing on 3–4. Loads content by path. States are a constant
/// 37 748 736 bytes; each save stops and restarts the emulator and writes the flash
/// (`dc_nvmem.bin`), so they are `Serialized` (no run-ahead / rewind).
pub const FLYCAST: CoreSpec = CoreSpec {
    id: "flycast",
    library_stem: "flycast_libretro",
    display_name: "Flycast",
    systems: &["dreamcast"],
    license: "GPL-2.0-or-later",
    fixed_options: FLYCAST_OPTIONS,
    firmware: FLYCAST_BIOS,
    bios_switch: Some(BiosOptionSwitch {
        key: "reicast_hle_bios",
        real: "disabled",
        hle: "enabled",
    }),
    memory_cards: &[],
    savestates: SaveStateQuality::Serialized,
    port_devices: &[JOYPAD, JOYPAD, NONE, NONE],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// Flycast options (prefix `reicast_`). Answered: every key whose C fallback differs
/// from its declared default (DSP, frame swapping, stick deadzone, anisotropic
/// filtering, OIT layers, DCNet), plus Nostyra's choices: internal resolution 2×
/// (1280×960, readback p95 1.3 ms), one emulation thread (all callbacks on the
/// caller's thread, frame-exact states), region from the disc, HLE BIOS unless a real
/// one exists, one VMU per game and pad (`All VMUs`), no rumble pack (deferred).
/// Never `per-pixel` alpha sorting: it asks for GL 4.3.
pub(crate) const FLYCAST_OPTIONS: &[(&str, &str)] = &[
    ("reicast_internal_resolution", "1280x960"),
    ("reicast_threaded_rendering", "disabled"),
    ("reicast_region", "Default"),
    ("reicast_language", "English"),
    ("reicast_broadcast", "NTSC"),
    ("reicast_cable_type", "TV (Composite)"),
    ("reicast_hle_bios", "enabled"),
    ("reicast_enable_dsp", "enabled"),
    ("reicast_alpha_sorting", "per-triangle (normal)"),
    ("reicast_delay_frame_swapping", "enabled"),
    ("reicast_detect_vsync_swap_interval", "disabled"),
    ("reicast_auto_skip_frame", "disabled"),
    ("reicast_frame_skipping", "disabled"),
    ("reicast_anisotropic_filtering", "4"),
    ("reicast_oit_layers", "32"),
    ("reicast_widescreen_hack", "disabled"),
    ("reicast_widescreen_cheats", "disabled"),
    ("reicast_gdrom_fast_loading", "disabled"),
    ("reicast_custom_textures", "disabled"),
    ("reicast_emulate_bba", "disabled"),
    ("reicast_dcnet", "disabled"),
    ("reicast_analog_stick_deadzone", "15%"),
    ("reicast_trigger_deadzone", "0%"),
    ("reicast_digital_triggers", "disabled"),
    ("reicast_per_content_vmus", "All VMUs"),
    ("reicast_device_port1_slot1", "VMU"),
    ("reicast_device_port1_slot2", "None"),
    ("reicast_device_port2_slot1", "VMU"),
    ("reicast_device_port2_slot2", "None"),
    ("reicast_device_port3_slot1", "VMU"),
    ("reicast_device_port3_slot2", "None"),
    ("reicast_device_port4_slot1", "VMU"),
    ("reicast_device_port4_slot2", "None"),
];
