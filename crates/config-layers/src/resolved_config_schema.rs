//! Config schema: typed structs with built-in defaults. Per-key validation of
//! raw layers lives in `known_key_checks`; the binding sections in
//! `input_binding_schema`, the resume section in `resume_config_schema`, the
//! achievements section in `achievements_config`.

use std::ops::RangeInclusive;
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::achievements_config::AchievementsConfig;
use crate::core_layer_id::BUNDLED_PRESETS;
use crate::input_binding_schema::InputConfig;
use crate::resume_config_schema::ResumeConfig;

/// `video.shader` value for the plain nearest-neighbour quad (no shader chain).
pub const SHADER_NONE: &str = "none";
/// Upper bound of `latency.run_ahead_frames`.
pub const MAX_RUN_AHEAD_FRAMES: u8 = 2;
/// Allowed `rewind.buffer_mb`.
pub const REWIND_BUFFER_MB_RANGE: RangeInclusive<u32> = 16..=512;

/// Effective settings after merging every layer (and, in the runner, CLI flags).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedConfig {
    pub video: VideoConfig,
    pub latency: LatencyConfig,
    pub rewind: RewindConfig,
    pub resume: ResumeConfig,
    /// Global layer only (`achievements_config`).
    pub achievements: AchievementsConfig,
    /// Binding sections, flattened so they sit at the top level
    /// (`input_keyboard`, …) as in the TOML layers and `--print-config`.
    #[serde(flatten)]
    pub input: InputConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    /// `none`, a [`BUNDLED_PRESETS`] name, or an absolute `.slangp` path.
    pub shader: String,
    pub scaling: VideoScaling,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            shader: SHADER_NONE.to_string(),
            scaling: VideoScaling::default(),
        }
    }
}

/// How the game picture fills the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoScaling {
    /// Largest size with the core's display aspect.
    #[default]
    Aspect,
    /// Aspect fit with an integer height multiple (sharp scanlines).
    Integer,
    /// Fill the window, ignoring aspect.
    Stretch,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LatencyConfig {
    /// 0 (off) ..= [`MAX_RUN_AHEAD_FRAMES`].
    pub run_ahead_frames: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RewindConfig {
    pub enabled: bool,
    /// History budget in MiB, within [`REWIND_BUFFER_MB_RANGE`].
    pub buffer_mb: u32,
}

impl Default for RewindConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            buffer_mb: 64,
        }
    }
}

/// `none`, a bundled preset name, or an absolute path to a `.slangp` preset.
pub fn is_valid_shader(value: &str) -> bool {
    let path = Path::new(value);
    value == SHADER_NONE
        || BUNDLED_PRESETS.contains(&value)
        || (path.is_absolute()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("slangp")))
}

/// Typed config from the merged (already validated) table.
pub(crate) fn to_resolved(merged: Table) -> ResolvedConfig {
    Value::Table(merged).try_into().unwrap_or_else(|err| {
        // Unreachable after validation; defaults keep the game playable anyway.
        tracing::warn!("cannot read merged config, using defaults: {err}");
        ResolvedConfig::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_values() {
        assert!(is_valid_shader("none") && is_valid_shader("crt-geom"));
        assert!(is_valid_shader("zfast-lcd"));
        assert!(!is_valid_shader("crt-royale") && !is_valid_shader("relative/x.slangp"));
        let abs = std::env::temp_dir().join("my.SLANGP");
        assert!(is_valid_shader(&abs.to_string_lossy()));
        assert!(!is_valid_shader(
            &abs.with_extension("glslp").to_string_lossy()
        ));
    }
}
