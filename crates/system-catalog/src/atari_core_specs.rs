//! Cores of the Atari systems (`atari_system_specs`): Stella (2600), ProSystem
//! (7800), Handy (Lynx). Small software cores from the libretro buildbot, never
//! bundled; none needs a BIOS (a ProSystem `7800 BIOS (U/E).rom` or a Handy
//! `lynxboot.img` copied into `<data>/system/` by hand is used silently).

use crate::core_spec_types::{CoreSpec, SaveStateQuality, CORE_DEFAULTS};

/// Stella 8 (Atari 2600): XRGB8888, RIOT RAM mapped for RetroAchievements, no
/// SAVE_RAM. Variable-size states (1 001 B for the test ROM, 1 MiB in the fast
/// savestate context). Frame-exact from the second frame after a load (the first
/// shows stale rows, M17 spike), so run-ahead / rewind are allowed.
pub const STELLA: CoreSpec = CoreSpec {
    id: "stella",
    library_stem: "stella_libretro",
    display_name: "Stella",
    systems: &["atari2600"],
    license: "GPL-2.0-or-later",
    savestates: SaveStateQuality::Deterministic,
    ..CORE_DEFAULTS
};

/// ProSystem 1.3e (Atari 7800): fixed-size states (49 221 B, 83 968 B in the fast
/// savestate context; the host sizes each context apart), refuses a wrong-size state
/// itself. Frame-exact.
pub const PROSYSTEM: CoreSpec = CoreSpec {
    id: "prosystem",
    library_stem: "prosystem_libretro",
    display_name: "ProSystem",
    systems: &["atari7800"],
    license: "GPL-2.0",
    savestates: SaveStateQuality::Deterministic,
    ..CORE_DEFAULTS
};

/// Handy 0.97 (Atari Lynx): HLE boot without `lynxboot.img`, rotates vertical games
/// itself (`SET_GEOMETRY` 102×160). `handy_refresh_rate` falls back to 75 Hz in the
/// core's C code (declared default 60), so 60 is always answered. Re-reads its content
/// from the path (staged, `stage_content`) and writes the cartridge EEPROM as
/// `<save folder>/content.eeprom` at reset and deinit. States: `SINGLE_SESSION`
/// (refused across core versions) and a fixed size per game; the core "loads" any
/// blob, even zeros or a truncated file, so other sizes are refused up front. One
/// stale frame right after a load, then frame-exact (M17 spike).
pub const HANDY: CoreSpec = CoreSpec {
    id: "handy",
    library_stem: "handy_libretro",
    display_name: "Handy",
    systems: &["lynx"],
    license: "Zlib",
    fixed_options: &[("handy_refresh_rate", "60")],
    savestates: SaveStateQuality::Deterministic,
    exact_state_size: true,
    strict_state_version: true,
    stage_content: true,
    ..CORE_DEFAULTS
};
