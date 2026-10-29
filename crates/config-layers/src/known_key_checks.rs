//! Every known `section.key` of the schema with its value check, applied to
//! each raw layer before merging, so one bad value only drops that key (the
//! lower layer or default shows through), never a section. All keys are
//! two-level (`split_once('.')`), including the binding sections.

use std::path::Path;
use std::sync::LazyLock;

use toml::{Table, Value};

use crate::achievements_config::ACHIEVEMENTS_KEY_CHECKS;
use crate::input_binding_schema::{binding_key_checks, drop_unknown_binding_keys};
use crate::resolved_config_schema::{
    is_valid_shader, MAX_RUN_AHEAD_FRAMES, REWIND_BUFFER_MB_RANGE,
};
use crate::resume_config_schema::RESUME_KEY_CHECKS;

/// A known key: `section`, `key` and its value check.
pub(crate) type KeyCheck = (&'static str, &'static str, fn(&Value) -> bool);

/// The pause-menu option keys (M3): what "Save for this game / as default" and
/// "Reset game settings" act on. Controls and resume keys are launcher-edited
/// and must survive a menu reset.
pub const MENU_OPTION_KEYS: [&str; 5] = [
    "video.shader",
    "video.scaling",
    "latency.run_ahead_frames",
    "rewind.enabled",
    "rewind.buffer_mb",
];

/// Checks of the menu option keys, in [`MENU_OPTION_KEYS`] order.
const MENU_OPTION_KEY_CHECKS: [KeyCheck; 5] = [
    ("video", "shader", |v| {
        v.as_str().is_some_and(is_valid_shader)
    }),
    ("video", "scaling", |v| {
        v.as_str()
            .is_some_and(|s| ["aspect", "integer", "stretch"].contains(&s))
    }),
    ("latency", "run_ahead_frames", |v| {
        v.as_integer()
            .is_some_and(|n| (0..=i64::from(MAX_RUN_AHEAD_FRAMES)).contains(&n))
    }),
    ("rewind", "enabled", |v| v.is_bool()),
    ("rewind", "buffer_mb", |v| {
        v.as_integer().is_some_and(|n| {
            (i64::from(*REWIND_BUFFER_MB_RANGE.start())..=i64::from(*REWIND_BUFFER_MB_RANGE.end()))
                .contains(&n)
        })
    }),
];

/// Every known key in schema order: menu options, resume, achievements, then the binding
/// sections (built from the name tables, hence not a `const`).
pub(crate) fn key_checks() -> &'static [KeyCheck] {
    static CHECKS: LazyLock<Vec<KeyCheck>> = LazyLock::new(|| {
        MENU_OPTION_KEY_CHECKS
            .into_iter()
            .chain(RESUME_KEY_CHECKS)
            .chain(ACHIEVEMENTS_KEY_CHECKS)
            .chain(binding_key_checks())
            .collect()
    });
    &CHECKS
}

/// `Some(valid)` for a known dotted key (`"video.shader"`), `None` for unknown keys.
pub(crate) fn check_known_key(dotted_key: &str, value: &Value) -> Option<bool> {
    let (section, key) = dotted_key.split_once('.')?;
    key_checks()
        .iter()
        .find(|(s, k, _)| (*s, *k) == (section, key))
        .map(|(_, _, valid)| valid(value))
}

/// Removes known keys with invalid values, unknown keys of binding sections and
/// known sections that are not tables from one raw layer, warning with the
/// layer's path. Unknown keys elsewhere stay.
pub(crate) fn drop_invalid_keys(layer: &mut Table, source: &Path) {
    for (section, key, valid) in key_checks() {
        let section_is_table = match layer.get_mut(*section) {
            None => continue,
            Some(Value::Table(table)) => {
                if table.get(*key).is_some_and(|value| !valid(value)) {
                    let value = table
                        .remove(*key)
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    tracing::warn!(layer = %source.display(), key = %format!("{section}.{key}"), %value,
                        "invalid config value ignored; using the lower layer or default");
                }
                true
            }
            Some(_) => false,
        };
        if !section_is_table {
            tracing::warn!(layer = %source.display(), %section, "config section is not a table; ignored");
            layer.remove(*section);
        }
    }
    drop_unknown_binding_keys(layer, source);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn array(items: &[&str]) -> Value {
        Value::Array(items.iter().map(|s| Value::from(*s)).collect())
    }

    #[test]
    fn known_key_checks() {
        assert_eq!(
            check_known_key("latency.run_ahead_frames", &Value::Integer(2)),
            Some(true)
        );
        assert_eq!(
            check_known_key("latency.run_ahead_frames", &Value::Integer(3)),
            Some(false)
        );
        assert_eq!(
            check_known_key("rewind.buffer_mb", &Value::Integer(15)),
            Some(false)
        );
        assert_eq!(
            check_known_key("rewind.enabled", &Value::String("yes".into())),
            Some(false)
        );
        assert_eq!(check_known_key("video.extra", &Value::Boolean(true)), None);
        assert_eq!(check_known_key("video", &Value::Boolean(true)), None);
    }

    #[test]
    fn resume_checks() {
        let mode = |s: &str| check_known_key("resume.mode", &Value::from(s));
        assert_eq!(mode("always"), Some(true));
        assert_eq!(mode("sometimes"), Some(false));
        assert_eq!(
            check_known_key("resume.auto_save", &Value::Boolean(false)),
            Some(true)
        );
    }

    #[test]
    fn binding_checks() {
        let check = |key: &str, items: &[&str]| check_known_key(key, &array(items));
        assert_eq!(check("input_keyboard.b", &["KeyZ", "Space"]), Some(true));
        assert_eq!(check("input_keyboard.b", &[]), Some(true), "[] = unbound");
        assert_eq!(check("input_keyboard.b", &["Nope"]), Some(false));
        assert_eq!(check("input_keyboard.b", &["Escape"]), Some(false));
        assert_eq!(
            check("hotkeys_keyboard.save_state", &["Escape"]),
            Some(false)
        );
        assert_eq!(check("input_keyboard.b", &["KeyZ", "KeyZ"]), Some(false));
        assert_eq!(
            check("input_keyboard.b", &["KeyA", "KeyB", "KeyC"]),
            Some(false)
        );
        assert_eq!(
            check("input_keyboard.b", &["south"]),
            Some(false),
            "wrong device"
        );
        assert_eq!(check("input_gamepad.a", &["guide"]), Some(false));
        assert_eq!(check("hotkeys_gamepad.pause_menu", &["guide"]), Some(false));
        assert_eq!(check("input_gamepad.start", &["back"]), Some(true));
        assert_eq!(check("hotkeys_gamepad.save_state", &["start"]), Some(false));
        assert_eq!(check("hotkeys_gamepad.save_state", &["back"]), Some(false));
        assert_eq!(
            check("hotkeys_gamepad.save_state", &["left_stick"]),
            Some(true)
        );
        assert_eq!(
            check_known_key("input_gamepad.a", &Value::from("east")),
            Some(false),
            "not an array"
        );
        assert_eq!(check("input_gamepad.turbo", &["east"]), None);
    }

    #[test]
    fn menu_option_keys_lead_the_schema() {
        let first: Vec<String> = key_checks()[..MENU_OPTION_KEYS.len()]
            .iter()
            .map(|(s, k, _)| format!("{s}.{k}"))
            .collect();
        assert_eq!(first, MENU_OPTION_KEYS);
    }
}
