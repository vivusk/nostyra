//! The runner's effective config: config layers (global → system → core → game)
//! resolved after the core loads (the core layer needs its `library_name`), then
//! the hidden CLI overrides on top. The loop keeps its own live copy (pause-menu
//! changes apply to this session only); [`EffectiveConfig`] remembers the layer
//! paths so the menu can write changed keys to a layer and re-resolve
//! (`layer_saves`).

use std::path::Path;

use config_layers::{
    core_layer_id, resolve_config, ConfigLayerPaths, ResolvedConfig, VideoScaling,
};
use video_renderer::ScalingMode;

use crate::ipc_event_writer::write_stdout;
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_data_paths::RunnerDataPaths;

mod layer_saves;
#[cfg(test)]
mod system_trigger_tests;

pub use layer_saves::SaveTarget;

/// CLI flags that win over every layer (kept so a re-resolve applies them again).
#[derive(Debug, Clone, Default)]
struct CliOverrides {
    shader: Option<String>,
    run_ahead: Option<u8>,
    rewind: Option<bool>,
}

impl CliOverrides {
    fn from_args(args: &RunnerCliArgs) -> Self {
        Self {
            shader: args.shader.clone(),
            run_ahead: args.run_ahead,
            rewind: args.rewind,
        }
    }
}

/// Layer files of this game + CLI overrides, and the config they resolve to.
pub struct EffectiveConfig {
    layers: ConfigLayerPaths,
    cli: CliOverrides,
    /// The game system's pad actions (input conflict rules, e.g. PS1 L2/R2 on triggers).
    system_actions: &'static [&'static str],
    /// What the files + CLI give right now (what the next launch would use).
    resolved: ResolvedConfig,
}

impl EffectiveConfig {
    /// Layers for this game and core, then CLI flags.
    pub fn resolve(
        paths: &RunnerDataPaths,
        rom: &Path,
        core_library_name: &str,
        args: &RunnerCliArgs,
    ) -> Self {
        let layers = ConfigLayerPaths::new(
            &paths.data_root,
            paths.system.id,
            &core_layer_id(core_library_name),
            rom,
            &paths.content_sha1,
        );
        let config = Self::from_layers(
            layers,
            CliOverrides::from_args(args),
            paths.system.pad_actions,
        );
        tracing::info!(config = ?config.resolved, "config resolved");
        config
    }

    fn from_layers(
        layers: ConfigLayerPaths,
        cli: CliOverrides,
        system_actions: &'static [&'static str],
    ) -> Self {
        let mut config = Self {
            layers,
            cli,
            system_actions,
            resolved: ResolvedConfig::default(),
        };
        config.reload();
        config
    }

    pub fn resolved(&self) -> &ResolvedConfig {
        &self.resolved
    }

    /// The game system's pad actions (bindings built from `resolved` use them too).
    pub fn system_actions(&self) -> &'static [&'static str] {
        self.system_actions
    }

    /// Re-reads every layer (after a menu write) and re-applies the CLI flags.
    fn reload(&mut self) {
        self.resolved = resolve_config(&self.layers, Some(self.system_actions));
        apply_cli_overrides(&mut self.resolved, &self.cli);
    }
}

fn apply_cli_overrides(config: &mut ResolvedConfig, cli: &CliOverrides) {
    if let Some(shader) = &cli.shader {
        config.video.shader.clone_from(shader);
    }
    if let Some(frames) = cli.run_ahead {
        config.latency.run_ahead_frames = frames;
    }
    if let Some(enabled) = cli.rewind {
        config.rewind.enabled = enabled;
    }
}

/// `--print-config`: the effective config as pretty JSON on stdout.
pub fn print(config: &ResolvedConfig, system_id: &str) -> anyhow::Result<()> {
    let mut json = serde_json::to_value(config)?;
    if let Some(object) = json.as_object_mut() {
        object.insert("system".into(), system_id.into());
    }
    write_stdout(&format!("{}\n", serde_json::to_string_pretty(&json)?));
    Ok(())
}

/// Presenter scaling for the `video.scaling` setting.
pub fn scaling_mode(scaling: VideoScaling) -> ScalingMode {
    match scaling {
        VideoScaling::Aspect => ScalingMode::AspectFit,
        VideoScaling::Integer => ScalingMode::Integer,
        VideoScaling::Stretch => ScalingMode::Stretch,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal

    use clap::Parser;

    use super::*;

    #[test]
    fn cli_flags_override_layers() {
        let mut config = ResolvedConfig::default();
        config.video.shader = "crt-geom".into();
        config.rewind.enabled = true;
        let args = RunnerCliArgs::try_parse_from([
            "nostyra-runner",
            "--core",
            "c",
            "--rom",
            "r",
            "--run-ahead",
            "2",
            "--rewind",
            "off",
        ])
        .unwrap();
        apply_cli_overrides(&mut config, &CliOverrides::from_args(&args));
        assert_eq!(config.video.shader, "crt-geom"); // no flag: layer value kept
        assert_eq!(config.latency.run_ahead_frames, 2);
        assert!(!config.rewind.enabled);
    }

    #[test]
    fn scaling_maps_to_presenter_modes() {
        assert_eq!(scaling_mode(VideoScaling::Aspect), ScalingMode::AspectFit);
        assert_eq!(scaling_mode(VideoScaling::Integer), ScalingMode::Integer);
        assert_eq!(scaling_mode(VideoScaling::Stretch), ScalingMode::Stretch);
    }
}
