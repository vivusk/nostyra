//! GPU resources for drawing one RGBA texture over a viewport: render pipeline,
//! nearest sampler, and the core-frame texture recreated whenever the core's frame
//! size changes. The overlay layer reuses the same pipeline with alpha blending.

/// Pipeline + sampler; created once per surface format.
pub(crate) struct QuadPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
}

impl QuadPipeline {
    /// `blend`: `None` replaces the target (core frame), `Some` composites (overlay).
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen_quad"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fullscreen_quad.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("textured_quad"),
            layout: None, // derived from the shader's single bind group
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        // Nearest: pixel-art stays crisp; smoothing/CRT is the shader stage's job.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("frame_nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }
}

/// Storage format of the core frame. Slang shaders expect gamma-encoded input, so
/// librashader samples the texture through this (non-sRGB) format.
const FRAME_STORAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Texture holding the current frame, with the plain quad's bind group.
pub(crate) struct FrameTexture {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    pub size: (u32, u32),
}

impl FrameTexture {
    /// `quad_view_format` must match the surface's sRGB-ness so the plain quad passes
    /// colors through unchanged; the stored texels are always `Rgba8Unorm`.
    pub fn new(
        device: &wgpu::Device,
        quad: &QuadPipeline,
        size: (u32, u32),
        quad_view_format: wgpu::TextureFormat,
    ) -> Self {
        let texture = create_rgba_texture(device, "core_frame", size, quad_view_format);
        let bind_group = quad_bind_group(device, quad, &texture, quad_view_format);
        Self {
            texture,
            bind_group,
            size,
        }
    }

    /// Uploads tightly packed RGBA8 rows (`size.0 * size.1 * 4` bytes).
    pub fn upload(&self, queue: &wgpu::Queue, rgba: &[u8]) {
        upload_rgba(queue, &self.texture, self.size, rgba);
    }
}

/// `Rgba8Unorm` texture viewable as `view_format` too. `COPY_SRC`: librashader
/// copies its input into history buffers for presets that read previous frames.
pub(crate) fn create_rgba_texture(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    view_format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let srgb_view = [view_format];
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: extent(size),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FRAME_STORAGE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: if view_format == FRAME_STORAGE_FORMAT {
            &[]
        } else {
            &srgb_view
        },
    })
}

/// Bind group sampling `texture` as `view_format` with the quad's nearest sampler.
pub(crate) fn quad_bind_group(
    device: &wgpu::Device,
    quad: &QuadPipeline,
    texture: &wgpu::Texture,
    view_format: wgpu::TextureFormat,
) -> wgpu::BindGroup {
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(view_format),
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("textured_quad"),
        layout: &quad.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&quad.sampler),
            },
        ],
    })
}

/// Uploads tightly packed RGBA8 rows covering the whole `size` texture.
pub(crate) fn upload_rgba(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: (u32, u32),
    rgba: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.0 * 4),
            rows_per_image: Some(size.1),
        },
        extent(size),
    );
}

fn extent((width, height): (u32, u32)) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
