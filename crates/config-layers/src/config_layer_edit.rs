//! Pure, in-memory edit of one layer table (no file access): what
//! `set_layer_values` applies to a file, and what the web edition applies to the
//! text it read through the File System Access API before writing it back.

use toml::{Table, Value};

use crate::known_key_checks::check_known_key;
use crate::ConfigError;

/// Sets (`Some`) or removes (`None`) each dotted key in `table`. Every key is checked
/// first: an invalid key or known-key value fails the whole call and leaves `table`
/// unchanged. Tables left empty by a removal are pruned.
pub fn edit_layer_table<K: AsRef<str>>(
    table: &mut Table,
    entries: &[(K, Option<Value>)],
) -> Result<(), ConfigError> {
    for (key, value) in entries {
        validate(key.as_ref(), value.as_ref())?;
    }
    let mut edited = table.clone();
    for (key, value) in entries {
        let dotted_key = key.as_ref();
        let parts: Vec<&str> = dotted_key.split('.').collect();
        match value {
            Some(value) => {
                let (last, parents) = parts
                    .split_last()
                    .ok_or_else(|| ConfigError::InvalidKey(dotted_key.to_string()))?;
                let parent = table_at(&mut edited, parents)
                    .ok_or_else(|| ConfigError::InvalidKey(dotted_key.to_string()))?;
                parent.insert((*last).to_string(), value.clone());
            }
            None => remove_and_prune(&mut edited, &parts),
        }
    }
    *table = edited;
    Ok(())
}

/// TOML text of a layer table (what a layer file holds after an edit).
pub fn layer_table_text(table: &Table) -> Result<String, ConfigError> {
    Ok(toml::to_string(table)?)
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
    fn edits_all_or_nothing_in_memory() {
        let mut table: Table = toml::from_str("[video]\nshader = \"crt-geom\"\n").unwrap();
        let keys = Value::Array(vec![Value::from("KeyX")]);
        edit_layer_table(
            &mut table,
            &[
                ("input_keyboard.a", Some(keys.clone())),
                ("video.shader", None),
            ],
        )
        .unwrap();
        assert_eq!(table.get("video"), None, "emptied table pruned");
        assert_eq!(table["input_keyboard"]["a"], keys);

        let before = table.clone();
        let escape = Value::Array(vec![Value::from("Escape")]);
        let err = edit_layer_table(
            &mut table,
            &[
                ("input_keyboard.x", Some(keys)),
                ("input_keyboard.y", Some(escape)),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { .. }), "{err}");
        assert!(edit_layer_table(&mut table, &[("a..b", None::<Value>)]).is_err());
        assert_eq!(table, before);
    }
}
