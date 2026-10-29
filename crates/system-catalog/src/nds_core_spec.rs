//! melonDS DS (Nintendo DS): software renderer (threaded), built-in BIOS and
//! generated firmware with direct boot, cartridge save in `SAVE_RAM` (M13 phase-00
//! spike on buildbot DLL 1.4.0).

use crate::core_fixed_options::MELONDS_OPTIONS;
use crate::core_specs::{CoreSpec, SaveStateQuality, CORE_DEFAULTS};

/// No firmware files (built-in system files only: encrypted dumps cannot boot). The
/// state size is fixed per game and stable across reset (~18 MiB + the cart save),
/// so mismatched states are refused up front; 128 MiB covers 64 MiB NAND carts.
/// States are picture-exact with the threaded renderer → `Deterministic`.
/// `MELONDSDS_SKIP_ERROR_SCREEN` makes a config error fail `retro_load_game`
/// instead of showing the core's error screen.
pub const MELONDS_DS: CoreSpec = CoreSpec {
    id: "melonds_ds",
    library_stem: "melondsds_libretro",
    display_name: "melonDS DS",
    systems: &["nds"],
    license: "GPL-3.0-or-later",
    fixed_options: MELONDS_OPTIONS,
    savestates: SaveStateQuality::Deterministic,
    exact_state_size: true,
    max_state_bytes: 128 * 1024 * 1024,
    process_env: &[("MELONDSDS_SKIP_ERROR_SCREEN", "1")],
    // DSi mode is not allowed in hardcore (rcheevos' melonDS rule).
    hardcore_required_options: &[("melonds_console_mode", "ds")],
    ..CORE_DEFAULTS
};
