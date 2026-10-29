//! Single owner of "which systems and cores exist": file rules, size limits, DAT
//! and thumbnail names, pad actions and the core that plays each system. Read by
//! game-library (scan), the runner (size limits, saves folder), the launcher (core
//! per game, UI labels), artwork-cache and core-manager — none hard-codes a system.
//!
//! How to add a system: add a [`SystemSpec`] to `system_specs.rs` (or the family
//! file: `handheld_system_specs.rs`, `nintendo_64_system_spec.rs`,
//! `sega_system_specs.rs`, `dreamcast_system_spec.rs`, `psp_system_spec.rs`,
//! `nds_system_spec.rs`, `gamecube_wii_system_specs.rs`, disc systems
//! `disc_system_specs.rs`) and to [`ALL_SYSTEMS`], add or extend a [`CoreSpec`] in
//! `core_specs.rs`, vendor its DAT + metadat files and add its `platforms` row in a
//! storage-db migration.
//!
//! Files are classified by content (extension → candidates → size → probe), never
//! by folder name (see [`classify`]).

#![deny(unsafe_code)]

mod classify_error;
mod content_probe_checks;
mod core_fixed_options;
mod core_spec_types;
mod core_specs;
mod disc_probe;
mod disc_system_specs;
mod dreamcast_core_spec;
mod dreamcast_system_spec;
mod firmware_lists;
mod firmware_status;
mod foreign_executable_probe;
mod gamecube_wii_core_spec;
mod gamecube_wii_system_specs;
mod handheld_system_specs;
mod mega_drive_smd_layout;
mod n64_byte_order;
mod nds_core_spec;
mod nds_header_probe;
mod nds_system_spec;
mod nintendo_64_system_spec;
mod psp_core_spec;
mod psp_system_spec;
mod rom_classifier;
mod rom_header_rules;
mod sega_system_specs;
mod snes_nes_system_specs;
mod system_specs;
mod touch_screen_rule;

#[cfg(test)]
mod ra_console_id_tests;

pub use core_specs::{
    core, core_for_system, systems_for_core, BiosOptionSwitch, CoreAssets, CoreSpec, FirmwareFile,
    MemoryCardSlot, SaveStateQuality, CORES, CORE_DEFAULTS, DEFAULT_MAX_STATE_BYTES,
    GENESIS_PLUS_GX, MESEN, MGBA, MUPEN64PLUS_NEXT, PCSX_REARMED, SNES9X,
};
pub use disc_probe::{DiscProbe, DiscSignature, Media, NoDiscProbe};
pub use disc_system_specs::PSX;
pub use dreamcast_core_spec::FLYCAST;
pub use dreamcast_system_spec::DREAMCAST;
pub use firmware_status::{
    bios_regions, firmware_dir, is_valid_bios, scan_firmware, BiosChoice, BiosFileStatus,
    FirmwareReport, ANY_REGION, BIOS_BYTES, BIOS_HEAD_BYTES, BIOS_REGIONS,
};
pub use gamecube_wii_core_spec::DOLPHIN;
pub use gamecube_wii_system_specs::{GC, WII};
pub use handheld_system_specs::{GB, GBA, GBC};
#[doc(hidden)]
pub use mega_drive_smd_layout::raw_to_smd;
pub use mega_drive_smd_layout::rom_image_for_hash;
#[doc(hidden)]
pub use n64_byte_order::n64_to_order;
pub use n64_byte_order::{normalise_in_place, N64ByteOrder};
pub use nds_core_spec::MELONDS_DS;
pub use nds_header_probe::{crc16_modbus, NDS_DSI_ONLY};
pub use nds_system_spec::NDS;
pub use nintendo_64_system_spec::NINTENDO_64;
pub use psp_core_spec::PPSSPP;
pub use psp_system_spec::PSP;
pub use rom_classifier::{
    candidates_for_extension, classify, classify_among, classify_path, classify_with_head,
    read_probe_bytes, ClassifyError,
};
pub use rom_header_rules::{hash_skip_len, ines_header_len, ContentProbe, HeaderRule, PROBE_BYTES};
pub use sega_system_specs::{GENESIS, GG, SG1000, SMS};
pub use system_specs::{system, AnalogSticks, SystemSpec, ALL_SYSTEMS, DISPLAY_ORDER, NES, SNES};
pub use touch_screen_rule::{touch_rect, TouchRect, TouchScreenRule};
