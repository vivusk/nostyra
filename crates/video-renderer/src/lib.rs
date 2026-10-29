//! Video output: converts core frames to RGBA8 and presents them via wgpu.
//!
//! [`RgbaFrameBuffer`] is the `VideoSink` handed to the core each frame;
//! [`GpuFramePresenter`] uploads and draws it into a winit window, optionally
//! through a librashader shader preset ([`ShaderChoice`]), with an
//! [`OverlayCanvas`] (status text, menus, achievements) blended on top after the
//! shader; [`TtfText`] draws Unicode text (bundled Noto Sans) into it.

#![deny(unsafe_code)]

mod bitmap_font_5x7;
mod frame_texture_pipeline;
mod gpu_adapter_selection;
mod gpu_frame_presenter;
mod overlay_canvas;
mod overlay_glyph_cache;
mod overlay_layer;
mod overlay_ttf_text;
mod pixel_format_converter;
mod renderer_error;
mod rgba_frame_buffer;
#[cfg(test)]
mod shader_chain_gpu_tests;
mod shader_chain_loader;
mod shader_stage;
mod viewport_scaling;

pub use gpu_frame_presenter::GpuFramePresenter;
pub use overlay_canvas::{text_size, OverlayCanvas};
pub use overlay_ttf_text::{FontFace, TtfText};
pub use pixel_format_converter::convert_to_rgba;
pub use renderer_error::{PresentOutcome, RendererError};
pub use rgba_frame_buffer::RgbaFrameBuffer;
pub use shader_chain_loader::ShaderLoadError;
pub use shader_stage::ShaderChoice;
pub use viewport_scaling::{compute_viewport, ScalingMode, ViewportRect};
/// Re-exported so callers pick a present mode without depending on wgpu directly.
pub use wgpu::PresentMode;
