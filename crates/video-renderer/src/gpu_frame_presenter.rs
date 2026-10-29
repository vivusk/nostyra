//! Owns the wgpu surface/device for one window and presents `RgbaFrameBuffer`s.
//! A frame is drawn in two steps: the shader stage (plain quad or librashader chain)
//! into the game viewport, then the overlay layer blended over the whole window.

use std::sync::Arc;

use winit::window::Window;

use crate::frame_texture_pipeline::{FrameTexture, QuadPipeline};
use crate::gpu_adapter_selection::{open_surface_and_adapter, request_device};
use crate::overlay_layer::OverlayLayer;
use crate::renderer_error::{PresentOutcome, RendererError};
use crate::rgba_frame_buffer::RgbaFrameBuffer;
use crate::shader_chain_loader::{ShaderChainLoader, ShaderLoadError};
use crate::shader_stage::{ShaderStage, StageTargets};
use crate::viewport_scaling::{compute_viewport, ScalingMode};

mod presenter_settings;

pub struct GpuFramePresenter {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    config: wgpu::SurfaceConfiguration,
    supported_present_modes: Vec<wgpu::PresentMode>,
    quad: QuadPipeline,
    frame_texture: Option<FrameTexture>,
    /// Format plain quads sample textures as (sRGB when the surface is sRGB).
    quad_view_format: wgpu::TextureFormat,
    /// Non-sRGB view format of the surface, the shader chain's render target.
    chain_format: wgpu::TextureFormat,
    scaling: ScalingMode,
    aspect_ratio: f32,
    stage: ShaderStage,
    loader: ShaderChainLoader,
    overlay: OverlayLayer,
    /// Chain that failed while rendering; reported by the next `poll_shader_load`.
    failed_chain: Option<ShaderLoadError>,
}

impl GpuFramePresenter {
    /// Creates surface + device for `window` (backend choice: see `gpu_adapter_selection`;
    /// `gl_in_use`: a core renders through OpenGL in this process, so no wgpu GL backend).
    /// `present_mode` falls back to `Fifo` (always supported) when unavailable.
    pub fn new(
        window: Arc<Window>,
        present_mode: wgpu::PresentMode,
        gl_in_use: bool,
    ) -> Result<Self, RendererError> {
        let size = window.inner_size();
        let (surface, adapter) = open_surface_and_adapter(&window, gl_in_use)?;
        let (device, queue) = request_device(&adapter)?;
        let caps = surface.get_capabilities(&adapter);
        // sRGB surface + sRGB texture = core colors pass through unchanged.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or(RendererError::NoSurfaceFormat)?;
        let present_mode = Some(present_mode)
            .filter(|mode| caps.present_modes.contains(mode))
            .unwrap_or(wgpu::PresentMode::Fifo);
        // Opaque: shader passes may leave alpha < 1, which a blending compositor
        // would show as a see-through window.
        let alpha_mode = [wgpu::CompositeAlphaMode::Opaque]
            .into_iter()
            .chain(caps.alpha_modes.first().copied())
            .find(|mode| caps.alpha_modes.contains(mode))
            .unwrap_or_default();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: Default::default(),
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![format.remove_srgb_suffix()],
        };
        surface.configure(&device, &config);
        tracing::info!(adapter = ?adapter.get_info().name, backend = ?adapter.get_info().backend,
            ?format, ?present_mode, ?alpha_mode, alpha_modes = ?caps.alpha_modes, "GPU presenter ready");
        let quad_view_format = if format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        Ok(Self {
            quad: QuadPipeline::new(&device, format, None),
            overlay: OverlayLayer::new(&device, format, quad_view_format),
            surface,
            device,
            queue,
            adapter_info: adapter.get_info(),
            config,
            supported_present_modes: caps.present_modes,
            frame_texture: None,
            quad_view_format,
            chain_format: format.remove_srgb_suffix(),
            scaling: ScalingMode::default(),
            aspect_ratio: 0.0,
            stage: ShaderStage::Plain,
            loader: ShaderChainLoader::default(),
            failed_chain: None,
        })
    }

    /// Uploads the frame if it changed (recreating the texture on size change) and
    /// draws it. Dupe frames simply redraw the existing texture.
    pub fn render(&mut self, frame: &mut RgbaFrameBuffer) -> Result<PresentOutcome, RendererError> {
        if frame.take_dirty() {
            self.upload(frame);
        }
        let target = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(PresentOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(PresentOutcome::Skipped)
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RendererError::SurfaceValidation)
            }
        };
        let surface_view = target.texture.create_view(&Default::default());
        let chain_view = target.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.chain_format),
            ..Default::default()
        });
        let targets = StageTargets {
            surface_view: &surface_view,
            chain_view: &chain_view,
            chain_format: self.chain_format,
            size: (self.config.width, self.config.height),
        };
        let frame = self.frame_texture.as_ref().map(|tex| {
            let rect = compute_viewport(targets.size, tex.size, self.aspect_ratio, self.scaling);
            (tex, rect)
        });
        // wgpu's default handler panics on validation errors; capture those from a
        // bad chain so they fall back to the plain picture like librashader errors.
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let record = self.stage.record(&mut encoder, &targets, &self.quad, frame);
        if let Err(err) = record {
            drop_chain(&mut self.stage, &mut self.failed_chain, &err.to_string());
            // Redraw plainly: the partial chain output is cleared by this pass.
            self.stage
                .record(&mut encoder, &targets, &self.quad, frame)
                .ok();
        }
        self.overlay.record(&mut encoder, &surface_view);
        self.queue.submit([encoder.finish()]);
        if let Some(err) = pollster::block_on(scope.pop()) {
            // This frame is lost; the following ones draw plainly.
            drop_chain(
                &mut self.stage,
                &mut self.failed_chain,
                &format!("GPU validation error: {err}"),
            );
        }
        self.queue.present(target);
        Ok(PresentOutcome::Presented)
    }

    fn upload(&mut self, frame: &RgbaFrameBuffer) {
        let size = frame.size();
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        if self.frame_texture.as_ref().map(|t| t.size) != Some(size) {
            tracing::debug!(?size, "frame size changed; recreating texture");
            let tex = FrameTexture::new(&self.device, &self.quad, size, self.quad_view_format);
            self.frame_texture = Some(tex);
        }
        if let Some(tex) = &self.frame_texture {
            tex.upload(&self.queue, frame.pixels());
        }
    }
}

/// Switches to the plain stage, reporting a failed chain via `poll_shader_load`.
fn drop_chain(stage: &mut ShaderStage, failed: &mut Option<ShaderLoadError>, message: &str) {
    if let ShaderStage::Chain { preset, .. } = stage {
        tracing::error!(preset = %preset.display(), %message, "shader chain failed; plain picture");
        *failed = Some(ShaderLoadError {
            preset: preset.clone(),
            message: message.to_owned(),
        });
    }
    *stage = ShaderStage::Plain;
}
