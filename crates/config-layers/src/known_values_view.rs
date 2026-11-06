//! Flat, dotted-key view of the known settings (`{"video.shader": "crt-geom", …}`)
//! for the launcher: the effective config and what one layer sets, each key in the
//! same form `set_layer_value` takes, so a UI field maps 1:1 to a key. Binding
//! keys carry their array as is (`{"input_keyboard.b": ["KeyZ"]}`).

use std::path::Path;

use toml::{Table, Value};

#[cfg(feature = "fs")]
use crate::config_layer_reader::read_layer_table;
use crate::known_key_checks::drop_invalid_keys;
use crate::resolved_config_diff::{as_table, known_keys};
use crate::resolved_config_schema::ResolvedConfig;
#[cfg(feature = "fs")]
use crate::ConfigError;

/// Every known key with its effective value.
pub fn resolved_known_values(config: &ResolvedConfig) -> Table {
    flatten_known(&as_table(config))
}

/// The valid known keys one layer file sets (missing file → empty). Invalid values
/// are dropped (as resolution does) and unknown keys are left out.
#[cfg(feature = "fs")]
pub fn layer_known_values(path: &Path) -> Result<Table, ConfigError> {
    Ok(match read_layer_table(path)? {
        Some(layer) => known_values_of_layer(layer, path),
        None => Table::new(),
    })
}

/// [`layer_known_values`] of a parsed layer (`source` names it in warnings).
pub fn known_values_of_layer(mut layer: Table, source: &Path) -> Table {
    drop_invalid_keys(&mut layer, source);
    flatten_known(&layer)
}

/// Known keys present in a layer-shaped table, keyed by dotted name, schema order.
fn flatten_known(table: &Table) -> Table {
    known_keys()
        .filter_map(|dotted| {
            let (section, key) = dotted.split_once('.')?;
            let value: &Value = table.get(section)?.as_table()?.get(key)?;
            Some((dotted.clone(), value.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_view_has_every_key() {
        let view = resolved_known_values(&ResolvedConfig::default());
        assert_eq!(view.len(), known_keys().count());
        assert_eq!(view["video.shader"], Value::String("none".into()));
        assert_eq!(view["rewind.buffer_mb"], Value::Integer(64));
    }

    #[cfg(feature = "fs")]
    #[test]
    fn layer_view_keeps_valid_known_keys_only() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-known-")
            .tempdir()
            .unwrap();
        let dir = temp.path();
        let path = dir.join("game.toml");
        std::fs::write(
            &path,
            "extra = 1\n[video]\nshader = \"crt-geom\"\nfuture = true\n[latency]\nrun_ahead_frames = 9\n",
        )
        .unwrap();
        let view = layer_known_values(&path).unwrap();
        assert_eq!(view.len(), 1, "{view:?}");
        assert_eq!(view["video.shader"], Value::String("crt-geom".into()));
        assert!(layer_known_values(&dir.join("missing.toml"))
            .unwrap()
            .is_empty());
    }
}
