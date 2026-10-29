//! Core options vs hardcore (M15 phase 04): at session start every option the host
//! answers (registry fixed options, the BIOS switch, hidden `--core-option`) is
//! checked with `achievements::is_option_disallowed` (rcheevos' table by the core's
//! `library_name` + the registry's `hardcore_required_options`); a required option
//! the host does not answer counts too (the core default may not be the required
//! value). Any hit drops the session to softcore.

use achievements::is_option_disallowed;
use system_catalog::CoreSpec;

/// The first option hardcore does not allow, as `key=value` (`key unset` for a
/// required option missing from `options`); `None` when all pass or the system
/// has no registry core.
pub(super) fn disallowed_option(
    library_name: &str,
    core: Option<&CoreSpec>,
    options: &[(String, String)],
) -> Option<String> {
    let core = core?;
    let answered = |key: &str| options.iter().any(|(k, _)| k == key);
    if let Some((key, _)) = core
        .hardcore_required_options
        .iter()
        .find(|(key, _)| !answered(key))
    {
        return Some(format!("{key} unset"));
    }
    options
        .iter()
        .find(|(key, value)| is_option_disallowed(library_name, core, key, value))
        .map(|(key, value)| format!("{key}={value}"))
}

#[cfg(test)]
mod tests {
    use system_catalog::core_for_system;

    use super::*;
    use crate::runner_core_options::resolve;

    fn option(key: &str, value: &str) -> Vec<(String, String)> {
        vec![(key.to_owned(), value.to_owned())]
    }

    #[test]
    fn shipped_options_pass_and_injected_cheats_fail() {
        let snes = &system_catalog::SNES;
        let core = core_for_system(snes.id);
        assert_eq!(disallowed_option("Snes9x", core, &resolve(snes, &[])), None);
        let pal = resolve(snes, &option("snes9x_region", "pal"));
        assert_eq!(
            disallowed_option("Snes9x", core, &pal).as_deref(),
            Some("snes9x_region=pal")
        );
        let nes = &system_catalog::NES;
        let mesen = core_for_system(nes.id);
        let dendy = resolve(nes, &option("mesen_region", "Dendy"));
        assert!(disallowed_option("Mesen", mesen, &dendy).is_some());
    }

    #[test]
    fn melonds_ds_is_forced_to_ds_mode_by_the_registry() {
        let nds = &system_catalog::NDS;
        let core = core_for_system(nds.id);
        let key = core.map(|c| c.hardcore_required_options[0].0).unwrap_or("");
        let shipped = resolve(nds, &[]);
        assert!(
            shipped.iter().any(|(k, v)| k == key && v == "ds"),
            "registry forces DS mode: {shipped:?}"
        );
        assert_eq!(disallowed_option("melonDS DS", core, &shipped), None);
        let dsi = resolve(nds, &option(key, "dsi"));
        assert_eq!(
            disallowed_option("melonDS DS", core, &dsi),
            Some(format!("{key}=dsi"))
        );
        assert_eq!(
            disallowed_option("melonDS DS", core, &[]),
            Some(format!("{key} unset"))
        );
    }
}
