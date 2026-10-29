//! Core options RetroAchievements hardcore does not allow (data only; enforcement is
//! the runner's): rcheevos' table by `library_name`, plus the registry's
//! `CoreSpec::hardcore_required_options` (melonDS DS must stay in DS mode).

use system_catalog::CoreSpec;

use crate::ffi::rc_setting_allowed;

/// Whether `key = value` is disallowed in hardcore for `core`, whose loaded library
/// reports `library_name` (`retro_system_info`).
pub fn is_option_disallowed(library_name: &str, core: &CoreSpec, key: &str, value: &str) -> bool {
    let required = core
        .hardcore_required_options
        .iter()
        .find(|(required_key, _)| *required_key == key);
    if let Some((_, wanted)) = required {
        if !value.eq_ignore_ascii_case(wanted) {
            return true;
        }
    }
    !rc_setting_allowed(library_name, key, value)
}

#[cfg(test)]
#[path = "hardcore_rules_tests.rs"]
mod tests;
