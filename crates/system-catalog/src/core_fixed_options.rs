//! Core options the host always answers for a core ([`CoreSpec::fixed_options`](crate::CoreSpec)),
//! kept apart from the core list because some cores need long lists.

/// PCSX ReARMed: without an answer the core sets up no memory card at all; the
/// threaded GPU only costs serialize time.
pub(crate) const PCSX_REARMED_OPTIONS: &[(&str, &str)] = &[
    ("pcsx_rearmed_memcard1", "libretro"),
    ("pcsx_rearmed_memcard2", "shared"),
    ("pcsx_rearmed_bios", "auto"),
    ("pcsx_rearmed_gpu_thread_rendering", "disabled"),
];

/// Mupen64Plus-Next: every key here is mandatory, because an unanswered key falls
/// back to 0 in the core's C code, not to its declared default (pure interpreter,
/// a dead analog stick, every C-button on B, no controller pak; M10 phase-00 spike).
/// GLideN64 renders 960×720 (3×; readback p95 0.91 ms). ThreadedRenderer is pinned
/// off: it calls a RetroArch-private environment id. The texture-cache keys are
/// answered too even though no texture filter is on: `MaxTxCacheSize` 0 corrupts
/// the heap a few seconds into real games (Super Robot Taisen 64, 2026-10-26).
/// The other unlisted keys declare 0/off/None defaults, which match the C fallback.
pub(crate) const MUPEN64PLUS_NEXT_OPTIONS: &[(&str, &str)] = &[
    ("mupen64plus-rdp-plugin", "gliden64"),
    ("mupen64plus-rsp-plugin", "hle"),
    ("mupen64plus-cpucore", "dynamic_recompiler"),
    ("mupen64plus-43screensize", "960x720"),
    ("mupen64plus-aspect", "4:3"),
    ("mupen64plus-EnableNativeResFactor", "0"),
    ("mupen64plus-ThreadedRenderer", "False"),
    ("mupen64plus-EnableFBEmulation", "True"),
    ("mupen64plus-EnableCopyColorToRDRAM", "Async"),
    ("mupen64plus-EnableCopyDepthToRDRAM", "Software"),
    ("mupen64plus-EnableLODEmulation", "True"),
    ("mupen64plus-EnableTextureCache", "True"),
    ("mupen64plus-EnableShadersStorage", "True"),
    ("mupen64plus-EnableFragmentDepthWrite", "True"),
    ("mupen64plus-BilinearMode", "standard"),
    ("mupen64plus-HybridFilter", "True"),
    ("mupen64plus-EnableOverscan", "Enabled"),
    ("mupen64plus-FrameDuping", "False"),
    ("mupen64plus-astick-deadzone", "15"),
    ("mupen64plus-astick-sensitivity", "100"),
    ("mupen64plus-r-cbutton", "C1"),
    ("mupen64plus-l-cbutton", "C2"),
    ("mupen64plus-d-cbutton", "C3"),
    ("mupen64plus-u-cbutton", "C4"),
    ("mupen64plus-alt-map", "False"),
    ("mupen64plus-pak1", "memory"),
    ("mupen64plus-pak2", "none"),
    ("mupen64plus-pak3", "none"),
    ("mupen64plus-pak4", "none"),
    ("mupen64plus-MaxTxCacheSize", "8000"),
    ("mupen64plus-txCacheCompression", "True"),
    ("mupen64plus-txFilterIgnoreBG", "True"),
];

/// melonDS DS (prefix `melonds_`, 44 of 84 keys; the other 40 fall back to their
/// declared defaults, M13 phase-00 spike): DS mode, direct boot on the built-in BIOS
/// and generated firmware (paths `/notfound`, so no user file is ever picked), both
/// SD cards off (else a 4 GB image), software renderer (threaded), top-bottom ↔
/// left-right layouts on R3, cursor only while touching, built-in blow sample for
/// the mic, a fixed firmware user name (no OS login name in firmware or states), no
/// network, every OSD message off (else a status message each frame). Clock: host
/// time (`absolute` would need all six `melonds_start_time_absolute_*` keys).
pub(crate) const MELONDS_OPTIONS: &[(&str, &str)] = &[
    ("melonds_console_mode", "ds"),
    ("melonds_boot_mode", "direct"),
    ("melonds_sysfile_mode", "builtin"),
    ("melonds_firmware_nds_path", "/notfound"),
    ("melonds_firmware_dsi_path", "/notfound"),
    ("melonds_dsi_nand_path", "/notfound"),
    ("melonds_homebrew_sdcard", "disabled"),
    ("melonds_homebrew_readonly", "disabled"),
    ("melonds_homebrew_sync_sdcard_to_host", "disabled"),
    ("melonds_dsi_sdcard", "disabled"),
    ("melonds_dsi_sdcard_readonly", "disabled"),
    ("melonds_dsi_sdcard_sync_sdcard_to_host", "disabled"),
    ("melonds_slot2_device", "auto"),
    ("melonds_start_time_mode", "real"),
    ("melonds_jit_enable", "enabled"),
    ("melonds_jit_block_size", "32"),
    ("melonds_jit_branch_optimisations", "enabled"),
    ("melonds_jit_literal_optimisations", "enabled"),
    ("melonds_jit_fast_memory", "enabled"),
    ("melonds_render_mode", "software"),
    ("melonds_opengl_resolution", "1"),
    ("melonds_opengl_better_polygons", "enabled"),
    ("melonds_threaded_renderer", "enabled"),
    ("melonds_number_of_screen_layouts", "2"),
    ("melonds_screen_layout1", "top-bottom"),
    ("melonds_screen_layout2", "left-right"),
    ("melonds_screen_gap", "0"),
    ("melonds_secondary_screen_scale", "100"),
    ("melonds_show_cursor", "touching"),
    ("melonds_touch_mode", "auto"),
    ("melonds_mic_input", "blow"),
    ("melonds_mic_input_active", "hold"),
    ("melonds_audio_bitdepth", "auto"),
    ("melonds_audio_interpolation", "disabled"),
    ("melonds_firmware_language", "en"),
    ("melonds_firmware_username", "melonDS DS"),
    ("melonds_network_mode", "disabled"),
    ("melonds_show_unsupported_features", "disabled"),
    ("melonds_show_bios_warnings", "disabled"),
    ("melonds_show_current_layout", "disabled"),
    ("melonds_show_mic_state", "disabled"),
    ("melonds_show_camera_state", "disabled"),
    ("melonds_show_lid_state", "disabled"),
    ("melonds_show_sensor_reading", "disabled"),
];
