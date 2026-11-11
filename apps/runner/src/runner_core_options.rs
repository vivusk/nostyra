//! Core options the host answers for this session: the registry's fixed options of
//! the system's core (e.g. PCSX ReARMed memory cards), then hidden `--core-option
//! key=value` overrides (tests force `pcsx_rearmed_bios=HLE`). Every other option
//! keeps the core's default. Also the core's save-state quality (run-ahead / rewind
//! policy), resolved the same way. A netplay room of more than two players adds the
//! core's multitap options and port devices (`system_catalog::MultitapSpec`); local
//! play is a "room" of [`LOCAL_PLAYERS`] and never gets them.

use system_catalog::{SaveStateQuality, SystemSpec, DEFAULT_NETPLAY_PLAYERS};

/// Players of a local session: the core's usual devices and options.
pub const LOCAL_PLAYERS: u8 = DEFAULT_NETPLAY_PLAYERS;

/// Save-state quality of `system`'s core (`Deterministic` for a system without one).
pub fn savestate_quality(system: &SystemSpec) -> SaveStateQuality {
    system_catalog::core_for_system(system.id)
        .map_or(SaveStateQuality::Deterministic, |core| core.savestates)
}

/// Whether `system`'s core needs states of exactly its state size (registry flag).
pub fn exact_state_size(system: &SystemSpec) -> bool {
    system_catalog::core_for_system(system.id).is_some_and(|core| core.exact_state_size)
}

/// Largest save state of `system`'s core (registry; 64 MiB unless raised).
pub fn max_state_bytes(system: &SystemSpec) -> u64 {
    system_catalog::core_for_system(system.id)
        .map_or(system_catalog::DEFAULT_MAX_STATE_BYTES, |core| {
            core.max_state_bytes
        })
}

/// Whether `system`'s core needs its state size re-queried before every save.
pub fn state_size_requery(system: &SystemSpec) -> bool {
    system_catalog::core_for_system(system.id).is_some_and(|core| core.requery_state_size)
}

/// Whether `system`'s core library is never unmapped (registry; Dolphin).
pub fn keep_library_loaded(system: &SystemSpec) -> bool {
    system_catalog::core_for_system(system.id).is_some_and(|core| core.keep_library_loaded)
}

/// Whether the system's core always gets its cartridge as a staged file (Handy).
pub fn stage_content(system: &SystemSpec) -> bool {
    system_catalog::core_for_system(system.id).is_some_and(|core| core.stage_content)
}

/// Devices for `retro_set_controller_port_device` after load (empty: none sent).
#[cfg_attr(not(test), allow(dead_code))] // the runner resolves per room size
pub fn port_devices(system: &SystemSpec) -> &'static [u32] {
    port_devices_for(system, LOCAL_PLAYERS)
}

/// [`port_devices`] for a netplay room of `players` (multitap devices above 2).
pub fn port_devices_for(system: &SystemSpec, players: u8) -> &'static [u32] {
    system_catalog::core_for_system(system.id).map_or(&[], |core| {
        system_catalog::port_devices_for(core, system.id, players)
    })
}

#[cfg_attr(not(test), allow(dead_code))] // the runner resolves per room size
/// Fixed options of `system`'s core with `overrides` applied (same key replaced,
/// new keys appended).
pub fn resolve(system: &SystemSpec, overrides: &[(String, String)]) -> Vec<(String, String)> {
    resolve_for_players(system, LOCAL_PLAYERS, overrides)
}

/// [`resolve`] for a netplay room of `players`: the core's multitap options (rooms
/// above 2 players) follow the fixed options, before `overrides`. Every role
/// resolves the same list, so the options hash matches across peers.
pub fn resolve_for_players(
    system: &SystemSpec,
    players: u8,
    overrides: &[(String, String)],
) -> Vec<(String, String)> {
    let core = system_catalog::core_for_system(system.id);
    let fixed = core.map_or(&[][..], |c| c.fixed_options);
    let multitap = core.map_or(&[][..], |c| {
        system_catalog::multitap_options_for(c, system.id, players)
    });
    let mut options: Vec<(String, String)> = fixed
        .iter()
        .chain(multitap)
        .map(|&(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    for (key, value) in overrides {
        match options.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1.clone_from(value),
            None => options.push((key.clone(), value.clone())),
        }
    }
    options
}

/// `key=value` CLI argument.
pub fn parse_core_option(arg: &str) -> Result<(String, String), String> {
    match arg.split_once('=') {
        Some((key, value)) if !key.trim().is_empty() => {
            Ok((key.trim().to_owned(), value.trim().to_owned()))
        }
        _ => Err("expected KEY=VALUE".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_replace_fixed_options_and_append_new_ones() {
        let fixed = resolve(&system_catalog::PSX, &[]);
        assert!(fixed.contains(&("pcsx_rearmed_memcard1".into(), "libretro".into())));
        let forced = resolve(
            &system_catalog::PSX,
            &[
                ("pcsx_rearmed_bios".into(), "HLE".into()),
                ("pcsx_rearmed_region".into(), "NTSC".into()),
            ],
        );
        assert_eq!(forced.len(), fixed.len() + 1);
        assert!(forced.contains(&("pcsx_rearmed_bios".into(), "HLE".into())));
        assert!(!forced.contains(&("pcsx_rearmed_bios".into(), "auto".into())));
        assert!(resolve(&system_catalog::SNES, &[]).is_empty());
    }

    #[test]
    fn savestate_quality_comes_from_the_systems_core() {
        let quality = savestate_quality(&system_catalog::NINTENDO_64);
        assert_eq!(quality, SaveStateQuality::Serialized);
        let quality = savestate_quality(&system_catalog::SNES);
        assert_eq!(quality, SaveStateQuality::Deterministic);
        assert!(exact_state_size(&system_catalog::NINTENDO_64));
        assert!(
            !exact_state_size(&system_catalog::DREAMCAST),
            "Flycast: unguarded"
        );
        assert!(!exact_state_size(&system_catalog::PSX));
        assert_eq!(port_devices(&system_catalog::DREAMCAST), [1, 1, 0, 0]);
        assert!(keep_library_loaded(&system_catalog::GC));
        assert!(!keep_library_loaded(&system_catalog::PSP));
        assert!(port_devices(&system_catalog::PSX).is_empty());
    }

    #[test]
    fn multitap_setup_only_for_rooms_above_two_players() {
        use system_catalog::{GENESIS, PSX, SNES};
        assert_eq!(port_devices(&SNES), port_devices_for(&SNES, 2));
        assert!(port_devices(&SNES).is_empty(), "local play unchanged");
        assert_eq!(port_devices_for(&SNES, 4), [1, 257]);
        assert_eq!(port_devices_for(&GENESIS, 4), [1537, 1]);
        assert_eq!(resolve_for_players(&PSX, 2, &[]), resolve(&PSX, &[]));
        let room = resolve_for_players(&PSX, 4, &[]);
        let tap = ("pcsx_rearmed_multitap".to_owned(), "port 1".to_owned());
        assert_eq!(room.len(), resolve(&PSX, &[]).len() + 1);
        assert!(room.contains(&tap));
        let forced = ("pcsx_rearmed_multitap".to_owned(), "disabled".to_owned());
        let overridden = resolve_for_players(&PSX, 4, std::slice::from_ref(&forced));
        assert!(overridden.contains(&forced) && !overridden.contains(&tap));
    }

    #[test]
    fn parses_key_value_pairs() {
        assert_eq!(
            parse_core_option("pcsx_rearmed_bios=HLE"),
            Ok(("pcsx_rearmed_bios".into(), "HLE".into()))
        );
        assert_eq!(parse_core_option("a=b=c"), Ok(("a".into(), "b=c".into())));
        assert!(parse_core_option("novalue").is_err());
        assert!(parse_core_option("=x").is_err());
    }
}
