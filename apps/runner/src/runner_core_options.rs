//! Core options the host answers for this session: the registry's fixed options of
//! the system's core (e.g. PCSX ReARMed memory cards), then hidden `--core-option
//! key=value` overrides (tests force `pcsx_rearmed_bios=HLE`). Every other option
//! keeps the core's default. Also the core's save-state quality (run-ahead / rewind
//! policy), resolved the same way.

use system_catalog::{SaveStateQuality, SystemSpec};

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

/// Devices for `retro_set_controller_port_device` after load (empty: none sent).
pub fn port_devices(system: &SystemSpec) -> &'static [u32] {
    system_catalog::core_for_system(system.id).map_or(&[], |core| core.port_devices)
}

/// Fixed options of `system`'s core with `overrides` applied (same key replaced,
/// new keys appended).
pub fn resolve(system: &SystemSpec, overrides: &[(String, String)]) -> Vec<(String, String)> {
    let fixed = system_catalog::core_for_system(system.id).map_or(&[][..], |c| c.fixed_options);
    let mut options: Vec<(String, String)> = fixed
        .iter()
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
