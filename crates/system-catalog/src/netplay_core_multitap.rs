//! Four-player netplay setups per core (`CoreSpec.multitap`), from the M20 spike 00
//! device tables (upstream `retro_controller_description` lists, checked against
//! the ports each core queries after `retro_set_controller_port_device`).

use crate::netplay_spec_types::{
    MultitapSpec, DEVICE_GPGX_MASTER_TAP, DEVICE_GPGX_TEAM_PLAYER_3B, DEVICE_JOYPAD,
    DEVICE_JOYPAD_MULTITAP, SINGLE_PLAYER,
};

/// Base of a four-player room entry (each setup fills devices / options).
const FOUR_PLAYERS: MultitapSpec = MultitapSpec {
    systems: &[],
    devices: &[],
    options: &[],
    max_players: 4,
};

/// Snes9x: Multitap in SNES port 2 (libretro port 1); players on ports 0–3.
pub(crate) const SNES9X_MULTITAP: &[MultitapSpec] = &[MultitapSpec {
    devices: &[DEVICE_JOYPAD, DEVICE_JOYPAD_MULTITAP],
    ..FOUR_PLAYERS
}];

/// Mesen: pads on ports 2–3 turn the NES Four Score on (NES carts; the Famicom
/// four-player adapter on port 4 is untested).
pub(crate) const MESEN_MULTITAP: &[MultitapSpec] = &[MultitapSpec {
    devices: &[
        DEVICE_JOYPAD,
        DEVICE_JOYPAD,
        DEVICE_JOYPAD_MULTITAP,
        DEVICE_JOYPAD_MULTITAP,
    ],
    ..FOUR_PLAYERS
}];

/// PCSX ReARMed: an option only (the core polls all 8 ports); a real 4-pad game
/// read is a manual check.
pub(crate) const PCSX_REARMED_MULTITAP: &[MultitapSpec] = &[MultitapSpec {
    options: &[("pcsx_rearmed_multitap", "port 1")],
    ..FOUR_PLAYERS
}];

/// Genesis Plus GX. Mega Drive: Sega Team Player (3 buttons) in port A, players on
/// ports 0–3 (EA 4-Way Play `[1025, 1025]` would be a per-game override, later).
/// Master System: Master Tap. Game Gear: one unit, one player. SG-1000: 2 pads.
pub(crate) const GENESIS_PLUS_GX_MULTITAP: &[MultitapSpec] = &[
    MultitapSpec {
        systems: &["genesis"],
        devices: &[DEVICE_GPGX_TEAM_PLAYER_3B, DEVICE_JOYPAD],
        ..FOUR_PLAYERS
    },
    MultitapSpec {
        systems: &["sms"],
        devices: &[DEVICE_GPGX_MASTER_TAP, DEVICE_JOYPAD],
        ..FOUR_PLAYERS
    },
    MultitapSpec {
        systems: &["gg"],
        ..SINGLE_PLAYER
    },
];
