//! Edits keys of one layer file (launcher settings, in-game "save for this
//! game"). The file is rewritten atomically; unknown keys survive, comments and
//! formatting do not. The edit itself is [`edit_layer_table`] (pure).

use std::path::Path;

use toml::Value;

use crate::atomic_file_write::write_atomically;
use crate::config_layer_edit::{edit_layer_table, layer_table_text};
use crate::config_layer_reader::read_layer_table;
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
    let mut table = read_layer_table(path)?.unwrap_or_default();
    edit_layer_table(&mut table, entries)?;
    let text = layer_table_text(&table)?;
    let io_error = |source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_error)?;
    }
    write_atomically(path, text.as_bytes()).map_err(io_error)
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
