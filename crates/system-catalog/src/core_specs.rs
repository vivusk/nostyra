//! The libretro cores Nostyra can download and run. One core may serve several
//! systems (add the system id to `systems`).

use crate::core_fixed_options::{MUPEN64PLUS_NEXT_OPTIONS, PCSX_REARMED_OPTIONS};
pub use crate::core_spec_types::{
    BiosOptionSwitch, CoreAssets, CoreSpec, FirmwareFile, MemoryCardSlot, SaveStateQuality,
    CORE_DEFAULTS, DEFAULT_MAX_STATE_BYTES,
};
use crate::dreamcast_core_spec::FLYCAST;
use crate::firmware_lists::PCSX_REARMED_BIOS;
use crate::gamecube_wii_core_spec::DOLPHIN;
use crate::nds_core_spec::MELONDS_DS;
use crate::psp_core_spec::PPSSPP;
use crate::system_specs::{system, SystemSpec, ALL_SYSTEMS};

/// Snes9x (SNES).
pub const SNES9X: CoreSpec = CoreSpec {
    id: "snes9x",
    library_stem: "snes9x_libretro",
    display_name: "Snes9x",
    systems: &["snes"],
    license: "Snes9x license (non-commercial)",
    fixed_options: &[],
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// Mesen 0.9.9 (NES). Mesen2's buildbot build rejected every ROM (M6 phase-00
/// spike), so the NES-only legacy core is used.
pub const MESEN: CoreSpec = CoreSpec {
    id: "mesen",
    library_stem: "mesen_libretro",
    display_name: "Mesen",
    systems: &["nes"],
    license: "GPL-3.0",
    fixed_options: &[],
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// mGBA (Game Boy, Game Boy Color, Game Boy Advance). Plays GBA without a BIOS
/// file; keeps the GB MBC3 clock in `RETRO_MEMORY_RTC` (M7 phase-00 spike).
pub const MGBA: CoreSpec = CoreSpec {
    id: "mgba",
    library_stem: "mgba_libretro",
    display_name: "mGBA",
    systems: &["gb", "gbc", "gba"],
    license: "MPL-2.0",
    fixed_options: &[],
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// PCSX ReARMed (PlayStation). Runs without a BIOS (built-in HLE BIOS) and uses a
/// real one found in the system folder. Card 1 = `SAVE_RAM`, card 2 = a core file in
/// the game's save folder (M8 phase-00 spike).
pub const PCSX_REARMED: CoreSpec = CoreSpec {
    id: "pcsx-rearmed",
    library_stem: "pcsx_rearmed_libretro",
    display_name: "PCSX ReARMed",
    systems: &["psx"],
    license: "GPL-2.0-or-later",
    fixed_options: PCSX_REARMED_OPTIONS,
    firmware: PCSX_REARMED_BIOS,
    bios_switch: None,
    memory_cards: &[
        MemoryCardSlot::SaveRam,
        MemoryCardSlot::CoreFile("pcsx-card2.mcd"),
    ],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// Genesis Plus GX (Mega Drive / Genesis, Master System, Game Gear, SG-1000). No
/// BIOS needed; loads ROMs by path only (`need_fullpath`), picks the system by file
/// extension, keeps cart RAM in `SAVE_RAM` trimmed to the last non-0xFF byte (M9
/// phase-00 spike). Non-commercial licence: never bundled, fetched from the buildbot.
pub const GENESIS_PLUS_GX: CoreSpec = CoreSpec {
    id: "genesis_plus_gx",
    library_stem: "genesis_plus_gx_libretro",
    display_name: "Genesis Plus GX",
    systems: &["genesis", "sms", "gg", "sg1000"],
    license: "Genesis Plus GX license (non-commercial)",
    fixed_options: &[],
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    ..CORE_DEFAULTS
};

/// Mupen64Plus-Next (Nintendo 64): GLideN64 on an OpenGL context the host provides,
/// HLE RSP, dynarec. Loads ROMs from memory, keeps EEPROM / SRAM / Flash and four
/// controller paks in one 296,960-byte `SAVE_RAM`. Serialize runs the emulation to
/// a safe point, so its states are not frame-exact (M10 phase-00 spike).
pub const MUPEN64PLUS_NEXT: CoreSpec = CoreSpec {
    id: "mupen64plus-next",
    library_stem: "mupen64plus_next_libretro",
    display_name: "Mupen64Plus-Next",
    systems: &["n64"],
    license: "GPL-2.0-or-later",
    fixed_options: MUPEN64PLUS_NEXT_OPTIONS,
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Serialized,
    port_devices: &[],
    exact_state_size: true,
    ..CORE_DEFAULTS
};

/// Every known core.
pub const CORES: [&CoreSpec; 10] = [
    &SNES9X,
    &MESEN,
    &MGBA,
    &PCSX_REARMED,
    &GENESIS_PLUS_GX,
    &MUPEN64PLUS_NEXT,
    &FLYCAST,
    &PPSSPP,
    &MELONDS_DS,
    &DOLPHIN,
];

/// Core by id.
pub fn core(id: &str) -> Option<&'static CoreSpec> {
    CORES.into_iter().find(|spec| spec.id == id)
}

/// Core that plays `system_id` (the system's `default_core_id`).
pub fn core_for_system(system_id: &str) -> Option<&'static CoreSpec> {
    system(system_id).and_then(|spec| core(spec.default_core_id))
}

/// Systems a core plays, in registry order.
pub fn systems_for_core(core_id: &str) -> Vec<&'static SystemSpec> {
    let Some(spec) = core(core_id) else {
        return Vec::new();
    };
    ALL_SYSTEMS
        .into_iter()
        .filter(|system| spec.systems.contains(&system.id))
        .collect()
}

#[cfg(test)]
#[path = "core_specs_tests.rs"]
mod tests;
