//! Core layer file id and the shader presets shipped with the app.

/// Shader presets bundled under `shaders/` (besides `none`): accepted by
/// `video.shader` validation and listed by the launcher's preset picker. Each has a
/// file path in [`bundled_preset_rel_path`].
pub const BUNDLED_PRESETS: &[&str] = &["crt-easymode", "crt-geom", "crt-lottes", "zfast-lcd"];

/// Path of a bundled preset relative to the shaders folder (the upstream
/// slang-shaders layout: CRT presets under `crt/`, LCD ones under `handheld/`);
/// `None` for an id not in [`BUNDLED_PRESETS`].
pub fn bundled_preset_rel_path(id: &str) -> Option<&'static str> {
    Some(match id {
        "crt-easymode" => "crt/crt-easymode.slangp",
        "crt-geom" => "crt/crt-geom.slangp",
        "crt-lottes" => "crt/crt-lottes.slangp",
        "zfast-lcd" => "handheld/zfast-lcd.slangp",
        _ => return None,
    })
}

/// File stem of the core layer (`core/<id>.toml`): the core's `library_name`
/// lower-cased, every char outside `[a-z0-9_-]` replaced by `_` (`"Snes9x"` →
/// `snes9x`, `"Mesen"` → `mesen`). The runner passes the loaded core's
/// `library_name`; the launcher passes the registry core id (`CoreSpec.id`), which
/// the system registry keeps equal to `core_layer_id(library_name)` (launcher/runner
/// parity test), so both pick the same file.
pub fn core_layer_id(library_name: &str) -> String {
    let id: String = library_name
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() {
        // A core without a name still gets a valid file name.
        "unknown".to_string()
    } else {
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_cases_and_replaces_other_chars() {
        assert_eq!(core_layer_id("Snes9x"), "snes9x");
        assert_eq!(core_layer_id("snes9x"), "snes9x");
        assert_eq!(core_layer_id("Beetle bSNES (HD)"), "beetle_bsnes__hd_");
        assert_eq!(core_layer_id("mGBA-2_x/é"), "mgba-2_x__");
        assert_eq!(core_layer_id(""), "unknown");
    }

    #[test]
    fn every_bundled_preset_has_a_path() {
        for id in BUNDLED_PRESETS {
            let path = bundled_preset_rel_path(id).unwrap_or_else(|| panic!("{id}"));
            assert!(path.ends_with(&format!("/{id}.slangp")), "{id}: {path}");
        }
        assert_eq!(bundled_preset_rel_path("none"), None);
        assert_eq!(bundled_preset_rel_path("../x"), None);
    }
}
