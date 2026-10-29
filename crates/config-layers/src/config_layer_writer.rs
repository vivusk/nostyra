//! Edits keys of one layer file (launcher settings, in-game "save for this
//! game"). The file is rewritten atomically; unknown keys survive, comments and
//! formatting do not.

use std::path::Path;

use toml::{Table, Value};

use crate::atomic_file_write::write_atomically;
use crate::config_layer_reader::read_layer_table;
use crate::known_key_checks::check_known_key;
use crate::ConfigError;

/// Sets (`Some`) or removes (`None`) `dotted_key` (e.g. `"video.shader"`) in the
/// layer at `path`; see [`set_layer_values`].
pub fn set_layer_value(
    path: &Path,
    dotted_key: &str,
    value: Option<Value>,
) -> Result<(), ConfigError> {
    set_layer_values(path, &[(dotted_key, value)])
}

/// Sets or removes several keys in **one** read-modify-write, so a multi-key edit
/// (launcher "steal" a binding, "Reset controls") is never half-applied. Creates
/// the file and its folders when needed; tables left empty by a removal are
/// pruned. Every key is checked before the file is touched: an invalid key or
/// known-key value fails the whole call, and a file that is not valid TOML is
/// left untouched (error) rather than overwritten.
pub fn set_layer_values<K: AsRef<str>>(
    path: &Path,
    entries: &[(K, Option<Value>)],
) -> Result<(), ConfigError> {
    for (key, value) in entries {
        validate(key.as_ref(), value.as_ref())?;
    }
    let mut table = read_layer_table(path)?.unwrap_or_default();
    for (key, value) in entries {
        let dotted_key = key.as_ref();
        let parts: Vec<&str> = dotted_key.split('.').collect();
        match value {
            Some(value) => {
                let (last, parents) = parts
                    .split_last()
                    .ok_or_else(|| ConfigError::InvalidKey(dotted_key.to_string()))?;
                let parent = table_at(&mut table, parents)
                    .ok_or_else(|| ConfigError::InvalidKey(dotted_key.to_string()))?;
                parent.insert((*last).to_string(), value.clone());
            }
            None => remove_and_prune(&mut table, &parts),
        }
    }
    let text = toml::to_string(&table)?;
    let io_error = |source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_error)?;
    }
    write_atomically(path, text.as_bytes()).map_err(io_error)
}

/// Rejects empty key segments and invalid values of known keys.
fn validate(dotted_key: &str, value: Option<&Value>) -> Result<(), ConfigError> {
    if dotted_key.split('.').any(str::is_empty) {
        return Err(ConfigError::InvalidKey(dotted_key.to_string()));
    }
    if let Some(value) = value {
        if check_known_key(dotted_key, value) == Some(false) {
            return Err(ConfigError::InvalidValue {
                key: dotted_key.to_string(),
                value: value.to_string(),
            });
        }
    }
    Ok(())
}

/// The nested table at `parts`, created as needed; `None` if a segment holds a
/// non-table value (we never silently replace user data with a table).
fn table_at<'a>(table: &'a mut Table, parts: &[&str]) -> Option<&'a mut Table> {
    let Some((first, rest)) = parts.split_first() else {
        return Some(table);
    };
    match table
        .entry((*first).to_string())
        .or_insert(Value::Table(Table::new()))
    {
        Value::Table(child) => table_at(child, rest),
        _ => None,
    }
}

/// Removes the key at `parts`, then drops every parent table left empty.
fn remove_and_prune(table: &mut Table, parts: &[&str]) {
    match parts {
        [] => {}
        [last] => {
            table.remove(*last);
        }
        [first, rest @ ..] => {
            if let Some(Value::Table(child)) = table.get_mut(*first) {
                remove_and_prune(child, rest);
                if child.is_empty() {
                    table.remove(*first);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn several_keys_in_one_write_and_all_or_nothing() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-multi-")
            .tempdir()
            .unwrap();
        let path = temp.path().join("global.toml");
        set_layer_value(&path, "video.shader", Some(Value::from("crt-geom"))).unwrap();
        let keys = Value::Array(vec![Value::from("KeyX")]);
        set_layer_values(
            &path,
            &[
                ("input_keyboard.a", Some(keys.clone())),
                ("input_keyboard.b", Some(Value::Array(vec![]))),
                ("video.shader", None),
            ],
        )
        .unwrap();
        let table = read_layer_table(&path).unwrap().unwrap();
        assert_eq!(table.get("video"), None, "emptied table pruned");
        assert_eq!(table["input_keyboard"]["a"], keys);
        assert_eq!(table["input_keyboard"]["b"], Value::Array(vec![]));

        // One bad entry (Escape is reserved) → nothing written.
        let before = std::fs::read_to_string(&path).unwrap();
        let escape = Value::Array(vec![Value::from("Escape")]);
        let err = set_layer_values(
            &path,
            &[
                ("input_keyboard.x", Some(keys)),
                ("input_keyboard.y", Some(escape)),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { .. }), "{err}");
        assert!(set_layer_values(&path, &[("a..b", None::<Value>)]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }
}
