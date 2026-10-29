//! Settings and per-frame inputs of the presenter besides the frame itself:
//! present mode, size, scaling, shader choice and the overlay canvas.

use crate::overlay_canvas::OverlayCanvas;
use crate::shader_chain_loader::ShaderLoadError;
use crate::shader_stage::{ShaderChoice, ShaderStage};
use crate::viewport_scaling::{compute_viewport, ScalingMode, ViewportRect};

use super::GpuFramePresenter;

impl GpuFramePresenter {
    /// Present mode actually in use (after fallback).
    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.config.present_mode
    }

    /// Switches to the first supported mode in `preferences` (else `Fifo`, which
    /// every surface supports) and returns the mode now in use.
    pub fn set_present_mode(&mut self, preferences: &[wgpu::PresentMode]) -> wgpu::PresentMode {
        let mode = preferences
            .iter()
            .copied()
            .find(|m| self.supported_present_modes.contains(m))
            .unwrap_or(wgpu::PresentMode::Fifo);
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
            tracing::info!(?mode, "present mode changed");
        }
        mode
    }

    /// Call on window resize (physical pixels). Zero sizes (minimized) are ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Display aspect from the core's AV info (`<= 0` = use frame w/h).
    pub fn set_aspect_ratio(&mut self, aspect: f32) {
        self.aspect_ratio = aspect;
    }

    /// The display aspect last set with [`Self::set_aspect_ratio`].
    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    pub fn set_scaling_mode(&mut self, mode: ScalingMode) {
        self.scaling = mode;
    }

    /// Switches the shader stage. `None` applies at once; a preset is built on a
    /// worker thread while the current stage keeps drawing, and `poll_shader_load`
    /// reports (and applies) the result.
    pub fn request_shader(&mut self, choice: ShaderChoice) {
        match choice {
            ShaderChoice::None => {
                self.loader.cancel();
                self.stage = ShaderStage::Plain;
            }
            ShaderChoice::Preset(preset) => {
                self.loader
                    .start(&self.device, &self.queue, self.adapter_info.clone(), preset);
            }
        }
    }

    /// Call once per frame: swaps in a finished chain and returns its preset path,
    /// or the error of a failed build / chain render (the plain picture stays up).
    pub fn poll_shader_load(&mut self) -> Option<Result<std::path::PathBuf, ShaderLoadError>> {
        if let Some(failed) = self.failed_chain.take() {
            return Some(Err(failed));
        }
        let (preset, chain) = match self.loader.poll()? {
            Ok(built) => built,
            Err(err) => return Some(Err(err)),
        };
        self.stage = ShaderStage::Chain {
            chain,
            preset: preset.clone(),
            frame_count: 0,
        };
        Some(Ok(preset))
    }

    /// Overlay drawn over the picture from the next `render` on (uploaded only when
    /// the canvas changed); `None` hides it. Size the canvas to `output_size`.
    pub fn set_overlay(&mut self, canvas: Option<&OverlayCanvas>) {
        self.overlay.set(&self.device, &self.queue, canvas);
    }

    /// Window surface size in physical pixels.
    pub fn output_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Where the game picture is drawn (physical pixels); `None` before the first frame.
    pub fn game_viewport(&self) -> Option<ViewportRect> {
        let tex = self.frame_texture.as_ref()?;
        Some(compute_viewport(
            self.output_size(),
            tex.size,
            self.aspect_ratio,
            self.scaling,
        ))
    }
}
