//! GPU side of the overlay: an output-sized RGBA texture alpha-blended over the
//! window after the shader stage, so CRT filters never touch UI. The texture is
//! re-uploaded only when the canvas changed (version or size).

use crate::frame_texture_pipeline::{
    create_rgba_texture, quad_bind_group, upload_rgba, QuadPipeline,
};
use crate::overlay_canvas::OverlayCanvas;

struct OverlayTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    version: u64,
}

pub(crate) struct OverlayLayer {
    quad: QuadPipeline,
    /// Format the overlay is sampled as (matches the surface's sRGB-ness).
    view_format: wgpu::TextureFormat,
    texture: Option<OverlayTexture>,
    visible: bool,
}

impl OverlayLayer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        view_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            quad: QuadPipeline::new(
                device,
                surface_format,
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
            view_format,
            texture: None,
            visible: false,
        }
    }

    /// Shows `canvas` from the next frame on (uploaded if it changed); `None` hides it.
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        canvas: Option<&OverlayCanvas>,
    ) {
        let Some(canvas) = canvas.filter(|c| c.size().0 > 0 && c.size().1 > 0) else {
            self.visible = false;
            return;
        };
        self.visible = true;
        let size = canvas.size();
        if self.texture.as_ref().map(|t| t.size) != Some(size) {
            let texture = create_rgba_texture(device, "overlay", size, self.view_format);
            let bind_group = quad_bind_group(device, &self.quad, &texture, self.view_format);
            self.texture = Some(OverlayTexture {
                texture,
                bind_group,
                size,
                version: canvas.version().wrapping_sub(1), // forces the first upload
            });
        }
        if let Some(tex) = self
            .texture
            .as_mut()
            .filter(|t| t.version != canvas.version())
        {
            upload_rgba(queue, &tex.texture, size, canvas.pixels());
            tex.version = canvas.version();
        }
    }

    /// Blends the overlay over the whole `target` (keeps what is already drawn).
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let Some(tex) = self.texture.as_ref().filter(|_| self.visible) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.quad.pipeline);
        pass.set_bind_group(0, &tex.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
