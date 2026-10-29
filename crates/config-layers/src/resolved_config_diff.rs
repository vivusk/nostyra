//! Which known keys differ between two resolved configs: the in-game menu's "Save
//! for this game / as default" writes only what the player changed (filtered to
//! [`crate::MENU_OPTION_KEYS`] by the runner), and "Reset game settings" removes
//! exactly the menu option keys from the game layer (other keys stay).

use toml::{Table, Value};

use crate::known_key_checks::key_checks;
use crate::resolved_config_schema::ResolvedConfig;

/// Every known key as a dotted name (`"video.shader"`, `"input_keyboard.b"`), in
/// schema order. Menu code wants [`crate::MENU_OPTION_KEYS`] instead.
pub fn known_keys() -> impl Iterator<Item = String> {
    key_checks()
        .iter()
        .map(|(section, key, _)| format!("{section}.{key}"))
}

/// `(dotted key, value in `to`)` for every known key whose value differs between
/// `from` and `to`, ready for `set_layer_value`.
pub fn changed_keys(from: &ResolvedConfig, to: &ResolvedConfig) -> Vec<(String, Value)> {
    let (from, to) = (as_table(from), as_table(to));
    key_checks()
        .iter()
        .filter_map(|(section, key, _)| {
            let new = leaf(&to, section, key)?;
            (leaf(&from, section, key) != Some(new))
                .then(|| (format!("{section}.{key}"), new.clone()))
        })
        .collect()
}

/// The config as a TOML table (same shape as a layer file).
pub(crate) fn as_table(config: &ResolvedConfig) -> Table {
    match Value::try_from(config) {
        Ok(Value::Table(table)) => table,
        // Unreachable: the schema is plain structs of strings, ints and bools.
        _ => Table::new(),
    }
}

fn leaf<'a>(table: &'a Table, section: &str, key: &str) -> Option<&'a Value> {
    table.get(section)?.as_table()?.get(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolved_config_schema::VideoScaling;

    #[test]
    fn only_changed_keys_with_their_new_values() {
        let from = ResolvedConfig::default();
        assert!(changed_keys(&from, &from).is_empty());
        let mut to = from.clone();
        to.video.scaling = VideoScaling::Integer;
        to.latency.run_ahead_frames = 1;
        assert_eq!(
            changed_keys(&from, &to),
            vec![
                ("video.scaling".to_string(), Value::String("integer".into())),
                ("latency.run_ahead_frames".to_string(), Value::Integer(1)),
            ]
        );
    }

    #[test]
    fn known_keys_cover_the_whole_schema() {
        let keys: Vec<String> = known_keys().collect();
        // 5 menu options + 2 resume + 5 achievements + 2 × 24 actions + 2 × 9 hotkeys.
        assert_eq!(keys.len(), 5 + 2 + 5 + 48 + 18);
        for key in [
            "rewind.buffer_mb",
            "resume.mode",
            "achievements.hardcore",
            "input_gamepad.r",
            "input_gamepad.l2",
            "hotkeys_gamepad.rewind_hold",
            "hotkeys_keyboard.turbo_a_toggle",
        ] {
            assert!(keys.contains(&key.to_string()), "{key}");
        }
    }
}
