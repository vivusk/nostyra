//! Netplay rules read from the registry: whether a core may run a rollback session
//! (`netplay_eligibility`), how many players a room of a system may have, and the
//! port devices / core options of that room. Pure functions of the spec, the system
//! and the room's `max_players`, so host, guests and spectators resolve the same
//! setup (they all apply it before `retro_load_game`).

use crate::core_spec_types::{CoreSpec, SaveStateQuality};
use crate::netplay_spec_types::{MultitapSpec, NetplaySupport, DEFAULT_NETPLAY_PLAYERS};

/// Why a core cannot run a netplay session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetplayRefusal {
    /// The registry does not offer netplay for this core.
    NotSupported,
    /// Serializing advances the emulation: rollback would desync.
    SerializedStates,
    /// The loaded core reports incomplete states (`SERIALIZATION_QUIRK_INCOMPLETE`).
    IncompleteStates,
}

/// Facts the runner learns from the core after `retro_load_game`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetplayQuirks {
    /// `SERIALIZATION_QUIRK_INCOMPLETE`.
    pub incomplete: bool,
}

/// Whether `spec` may run netplay. `quality` = the core's save-state quality
/// (`CoreSpec.savestates`). `quirks`: `None` for the launcher's registry-only
/// pre-check, the loaded core's facts for the runner's authoritative check.
pub fn netplay_eligibility(
    spec: &CoreSpec,
    quality: SaveStateQuality,
    quirks: Option<NetplayQuirks>,
) -> Result<NetplaySupport, NetplayRefusal> {
    if spec.netplay == NetplaySupport::No {
        return Err(NetplayRefusal::NotSupported);
    }
    if quality == SaveStateQuality::Serialized {
        return Err(NetplayRefusal::SerializedStates);
    }
    if quirks.is_some_and(|quirks| quirks.incomplete) {
        return Err(NetplayRefusal::IncompleteStates);
    }
    Ok(spec.netplay)
}

/// The multitap / room entry of `spec` for `system_id` (first match).
pub fn multitap_for(spec: &CoreSpec, system_id: &str) -> Option<&'static MultitapSpec> {
    spec.multitap
        .iter()
        .find(|entry| entry.systems.is_empty() || entry.systems.contains(&system_id))
}

/// Largest netplay room (players) of `system_id` on `spec`.
pub fn netplay_max_players(spec: &CoreSpec, system_id: &str) -> u8 {
    multitap_for(spec, system_id).map_or(DEFAULT_NETPLAY_PLAYERS, |entry| entry.max_players)
}

/// The room needs the multitap setup: more than two players and an entry that has
/// one.
fn multitap_in_use(
    spec: &CoreSpec,
    system_id: &str,
    max_players: u8,
) -> Option<&'static MultitapSpec> {
    multitap_for(spec, system_id).filter(|entry| {
        max_players > DEFAULT_NETPLAY_PLAYERS && entry.max_players > DEFAULT_NETPLAY_PLAYERS
    })
}

/// `retro_set_controller_port_device` list for a room of `max_players` (single
/// player / 2-player rooms: the core's usual `port_devices`).
pub fn port_devices_for(spec: &CoreSpec, system_id: &str, max_players: u8) -> &'static [u32] {
    match multitap_in_use(spec, system_id, max_players) {
        Some(entry) if !entry.devices.is_empty() => entry.devices,
        _ => spec.port_devices,
    }
}

/// Core options to merge into the session's options for a room of `max_players`
/// (empty unless the room uses the multitap).
pub fn multitap_options_for(
    spec: &CoreSpec,
    system_id: &str,
    max_players: u8,
) -> &'static [(&'static str, &'static str)] {
    multitap_in_use(spec, system_id, max_players).map_or(&[], |entry| entry.options)
}

#[cfg(test)]
#[path = "netplay_core_rules_tests.rs"]
mod tests;
