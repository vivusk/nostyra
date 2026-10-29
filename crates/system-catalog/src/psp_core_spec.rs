//! PPSSPP (Sony PSP): OpenGL hardware rendering (GL core 3.1 request, served by the
//! host's 3.3 core context; `ppsspp_backend=none` = software, tests only), system
//! files from the buildbot `PPSSPP.zip`, memory stick = the game's save folder
//! (M12 phase-00 spike on buildbot DLL 8a50821).

use crate::core_specs::{CoreAssets, CoreSpec, SaveStateQuality, CORE_DEFAULTS};

/// No firmware, no ports (`retro_set_controller_port_device` is a no-op). Release
/// asserts open a modal dialog unless `CI` is set. The state size grows after the
/// asynchronous boot without notice (128 MiB pre-boot, 40–72 MiB after) → re-queried
/// on every save, capped at 128 MiB. The emulator thread makes states not
/// frame-exact, so they are `Serialized` (no run-ahead / rewind).
pub const PPSSPP: CoreSpec = CoreSpec {
    id: "ppsspp",
    library_stem: "ppsspp_libretro",
    display_name: "PPSSPP",
    systems: &["psp"],
    license: "GPL-2.0-or-later",
    fixed_options: PPSSPP_OPTIONS,
    savestates: SaveStateQuality::Serialized,
    requery_state_size: true,
    max_state_bytes: 128 * 1024 * 1024,
    system_assets: Some(CoreAssets {
        dir: "PPSSPP",
        markers: &["compat.ini", "ppge_atlas.zim"],
        archive_path: "assets/system/PPSSPP.zip",
    }),
    process_env: &[("CI", "1")],
    ..CORE_DEFAULTS
};

/// All 70 PPSSPP options (prefix `ppsspp_`; the C fallback of an unanswered key often
/// differs from its declared default). Nostyra's choices: JIT (fallback: interpreter),
/// 2× internal resolution 960×544 (unanswered = 0×0 geometry), frame duplication to
/// 60 Hz with every frameskip off (else 30 fps games run 2–10× fast), one buffered
/// frame, 0.15 stick deadzone (no host deadzone), English, PSP-2000, no texture
/// replacement / cheats / networking, a fixed non-zero MAC (an all-zero one is
/// rewritten through 12 `SET_VARIABLE` calls).
pub(crate) const PPSSPP_OPTIONS: &[(&str, &str)] = &[
    ("ppsspp_cpu_core", "JIT"),
    ("ppsspp_fast_memory", "enabled"),
    ("ppsspp_ignore_bad_memory_access", "enabled"),
    ("ppsspp_io_timing_method", "Fast"),
    ("ppsspp_force_lag_sync", "disabled"),
    ("ppsspp_locked_cpu_speed", "disabled"),
    ("ppsspp_memstick_inserted", "enabled"),
    ("ppsspp_cache_iso", "disabled"),
    ("ppsspp_cheats", "disabled"),
    ("ppsspp_language", "English"),
    ("ppsspp_psp_model", "psp_2000_3000"),
    ("ppsspp_backend", "auto"),
    ("ppsspp_software_rendering", "disabled"),
    ("ppsspp_internal_resolution", "960x544"),
    ("ppsspp_mulitsample_level", "Disabled"),
    ("ppsspp_cropto16x9", "enabled"),
    ("ppsspp_frameskip", "disabled"),
    ("ppsspp_frameskiptype", "Number of frames"),
    ("ppsspp_auto_frameskip", "disabled"),
    ("ppsspp_frame_duplication", "enabled"),
    ("ppsspp_detect_vsync_swap_interval", "disabled"),
    ("ppsspp_inflight_frames", "Up to 1"),
    ("ppsspp_button_preference", "Cross"),
    ("ppsspp_analog_is_circular", "disabled"),
    ("ppsspp_analog_deadzone", "0.15"),
    ("ppsspp_analog_sensitivity", "1.00"),
    ("ppsspp_skip_buffer_effects", "disabled"),
    ("ppsspp_skip_gpu_readbacks", "disabled"),
    ("ppsspp_lower_resolution_for_effects", "disabled"),
    ("ppsspp_texture_scaling_type", "xbrz"),
    ("ppsspp_texture_scaling_level", "disabled"),
    ("ppsspp_texture_deposterize", "disabled"),
    ("ppsspp_texture_shader", "disabled"),
    ("ppsspp_texture_anisotropic_filtering", "16x"),
    ("ppsspp_texture_filtering", "Auto"),
    ("ppsspp_smart_2d_texture_filtering", "disabled"),
    ("ppsspp_texture_replacement", "disabled"),
    ("ppsspp_enable_wlan", "disabled"),
    ("ppsspp_change_mac_address01", "0"),
    ("ppsspp_change_mac_address02", "2"),
    ("ppsspp_change_mac_address03", "0"),
    ("ppsspp_change_mac_address04", "0"),
    ("ppsspp_change_mac_address05", "0"),
    ("ppsspp_change_mac_address06", "0"),
    ("ppsspp_change_mac_address07", "0"),
    ("ppsspp_change_mac_address08", "0"),
    ("ppsspp_change_mac_address09", "0"),
    ("ppsspp_change_mac_address10", "0"),
    ("ppsspp_change_mac_address11", "0"),
    ("ppsspp_change_mac_address12", "1"),
    ("ppsspp_wlan_channel", "Auto"),
    ("ppsspp_enable_builtin_pro_ad_hoc_server", "disabled"),
    ("ppsspp_change_pro_ad_hoc_server_address", "socom.cc"),
    ("ppsspp_pro_ad_hoc_server_address01", "0"),
    ("ppsspp_pro_ad_hoc_server_address02", "0"),
    ("ppsspp_pro_ad_hoc_server_address03", "0"),
    ("ppsspp_pro_ad_hoc_server_address04", "0"),
    ("ppsspp_pro_ad_hoc_server_address05", "0"),
    ("ppsspp_pro_ad_hoc_server_address06", "0"),
    ("ppsspp_pro_ad_hoc_server_address07", "0"),
    ("ppsspp_pro_ad_hoc_server_address08", "0"),
    ("ppsspp_pro_ad_hoc_server_address09", "0"),
    ("ppsspp_pro_ad_hoc_server_address10", "0"),
    ("ppsspp_pro_ad_hoc_server_address11", "0"),
    ("ppsspp_pro_ad_hoc_server_address12", "0"),
    ("ppsspp_enable_upnp", "disabled"),
    ("ppsspp_upnp_use_original_port", "enabled"),
    ("ppsspp_port_offset", "0"),
    ("ppsspp_minimum_timeout", "0"),
    ("ppsspp_forced_first_connect", "disabled"),
];
