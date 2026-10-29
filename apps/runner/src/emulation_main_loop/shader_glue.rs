//! Shader stage glue: resolves `video.shader` to a preset when the window opens and
//! reports finished / failed chain builds (log + on-screen status). The plain
//! picture keeps running whatever goes wrong.

use std::path::Path;
use std::time::Instant;

use video_renderer::ShaderChoice;

use super::EmulationApp;
use crate::shader_resource_dir::{candidate_dirs, resolve_shader};

impl EmulationApp<'_> {
    /// Asks the presenter for the configured shader (built on a worker thread).
    pub(super) fn request_configured_shader(&mut self) {
        let Some(presenter) = self.presenter.as_mut() else {
            return;
        };
        let dirs = candidate_dirs(self.shaders_dir.as_deref());
        match resolve_shader(&self.config.video.shader, &dirs) {
            Ok(choice) => {
                if let ShaderChoice::Preset(path) = &choice {
                    tracing::info!(preset = %path.display(), "loading shader preset");
                }
                presenter.request_shader(choice);
            }
            Err(err) => {
                tracing::warn!(shader = %self.config.video.shader, "{err}");
                self.status.show("Shaders not found", Instant::now());
            }
        }
    }

    /// Call once per redraw before presenting: applies a finished chain or shows
    /// why it failed.
    pub(super) fn report_shader_load(&mut self) {
        let Some(presenter) = self.presenter.as_mut() else {
            return;
        };
        match presenter.poll_shader_load() {
            Some(Ok(preset)) => tracing::info!(preset = %preset.display(), "shader active"),
            Some(Err(err)) => {
                tracing::error!("{err}");
                let name = preset_name(&err.preset);
                self.status
                    .show(format!("Shader failed: {name}"), Instant::now());
            }
            None => {}
        }
    }
}

/// Short name for the status line: the preset's file stem (`crt-geom`).
fn preset_name(preset: &Path) -> String {
    preset.file_stem().map_or_else(
        || preset.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}
