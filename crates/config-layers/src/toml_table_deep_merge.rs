//! Recursive TOML table merge: an overlay layer's tables merge key by key into
//! the base; any other value (scalar, array, or a type change) replaces it.

use toml::{Table, Value};

/// Merges `overlay` into `base`; `overlay` wins on conflicts.
pub(crate) fn deep_merge(base: &mut Table, overlay: Table) {
    for (key, value) in overlay {
        let value = match (base.get_mut(&key), value) {
            (Some(Value::Table(base_table)), Value::Table(overlay_table)) => {
                deep_merge(base_table, overlay_table);
                continue;
            }
            (_, value) => value,
        };
        base.insert(key, value);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // test code: panics are the failure signal
mod tests {
    use super::*;

    fn table(text: &str) -> Table {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn nested_tables_merge_and_scalars_arrays_replace() {
        let mut base =
            table("a = 1\nlist = [1, 2]\n[video]\nshader = \"none\"\nscaling = \"aspect\"");
        deep_merge(
            &mut base,
            table("list = [3]\n[video]\nshader = \"crt-geom\"\n[extra]\nx = true"),
        );
        assert_eq!(
            base,
            table(
                "a = 1\nlist = [3]\n[video]\nshader = \"crt-geom\"\nscaling = \"aspect\"\n\
                 [extra]\nx = true"
            )
        );
    }

    #[test]
    fn type_change_replaces_whole_value() {
        let mut base = table("[video]\nshader = \"none\"");
        deep_merge(&mut base, table("video = 3"));
        assert_eq!(base, table("video = 3"));
    }
}
