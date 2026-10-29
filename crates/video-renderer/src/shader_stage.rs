//! First draw step of a frame: the core frame into the game viewport, either with
//! the plain nearest-neighbour quad or through a librashader filter chain. Both
//! clear the whole window to black first (letterbox bars).

use std::path::PathBuf;

use librashader_common::{Size, Viewport};
use librashader_runtime_wgpu::error::FilterChainError;
use librashader_runtime_wgpu::{FilterChainWgpu, WgpuOutputView};

use crate::frame_texture_pipeline::{FrameTexture, QuadPipeline};
use crate::viewport_scaling::ViewportRect;

/// Which shader the presenter should use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShaderChoice {
    /// Plain nearest-neighbour picture (the default).
    None,
    /// A `.slangp` preset file, built on a worker thread.
    Preset(PathBuf),
}

pub(crate) enum ShaderStage {
    Plain,
    Chain {
        chain: Box<FilterChainWgpu>,
        preset: PathBuf,
        /// Frames presented with this chain (`FrameCount` uniform for animated passes).
        frame_count: usize,
    },
}

/// Views of the acquired surface texture. Slang shaders write gamma-encoded
/// values (like RetroArch's UNORM back buffer), so the chain renders through a
/// non-sRGB view; the plain quad keeps the surface's own (sRGB) view.
pub(crate) struct StageTargets<'a> {
    pub surface_view: &'a wgpu::TextureView,
    pub chain_view: &'a wgpu::TextureView,
    pub chain_format: wgpu::TextureFormat,
    pub size: (u32, u32),
}

impl ShaderStage {
    /// Records the frame (if any) into `targets`. A chain error leaves the window
    /// cleared but valid; the caller falls back to `Plain`.
    pub fn record(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &StageTargets<'_>,
        quad: &QuadPipeline,
        frame: Option<(&FrameTexture, ViewportRect)>,
    ) -> Result<(), Box<FilterChainError>> {
        match (self, frame) {
            (
                ShaderStage::Chain {
                    chain, frame_count, ..
                },
                Some((tex, rect)),
            ) => {
                let (x, y, width, height) = pixel_rect(rect, targets.size);
                let size = Size::new(width, height);
                let output =
                    WgpuOutputView::new_from_raw(targets.chain_view, size, targets.chain_format);
                let viewport = Viewport {
                    x: x as f32,
                    y: y as f32,
                    mvp: None,
                    output,
                    size,
                };
                // The final pass clears the whole attachment, then draws inside the rect.
                let result = chain.frame(&tex.texture, &viewport, encoder, *frame_count, None);
                *frame_count = frame_count.wrapping_add(1);
                result.map_err(Box::new)
            }
            (_, frame) => {
                record_plain(encoder, targets.surface_view, quad, frame);
                Ok(())
            }
        }
    }
}

fn record_plain(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    quad: &QuadPipeline,
    frame: Option<(&FrameTexture, ViewportRect)>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("present_frame"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    if let Some((tex, r)) = frame {
        pass.set_viewport(r.x, r.y, r.width, r.height, 0.0, 1.0);
        pass.set_pipeline(&quad.pipeline);
        pass.set_bind_group(0, &tex.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Whole-pixel rect inside `output` (librashader scissors to integer bounds).
fn pixel_rect(rect: ViewportRect, output: (u32, u32)) -> (u32, u32, u32, u32) {
    let x = (rect.x.max(0.0) as u32).min(output.0.saturating_sub(1));
    let y = (rect.y.max(0.0) as u32).min(output.1.saturating_sub(1));
    let width = (rect.width.round() as u32).clamp(1, output.0 - x);
    let height = (rect.height.round() as u32).clamp(1, output.1 - y);
    (x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_rect_rounds_and_stays_inside_the_output() {
        let r = ViewportRect {
            x: 160.0,
            y: 0.0,
            width: 959.6,
            height: 720.0,
        };
        assert_eq!(pixel_rect(r, (1280, 720)), (160, 0, 960, 720));
        let oversized = ViewportRect {
            x: -1.0,
            y: 5.0,
            width: 4000.0,
            height: 0.2,
        };
        assert_eq!(pixel_rect(oversized, (100, 50)), (0, 5, 100, 1));
    }
}
