//! GPU checks of the shader stage. `#[ignore]`: they need a real adapter (run with
//! `cargo test -p video-renderer -- --ignored --nocapture`). They use the same
//! backend preference and device features as the presenter, render offscreen into
//! an sRGB target viewed as UNORM for the chain (like the surface), and rely on
//! wgpu's default error handler, which panics on any validation error.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::frame_texture_pipeline::{FrameTexture, QuadPipeline};
use crate::gpu_adapter_selection::{candidate_backends, request_device};
use crate::shader_chain_loader::build_chain;
use crate::shader_stage::{ShaderStage, StageTargets};
use crate::viewport_scaling::ViewportRect;

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> Gpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = candidate_backends(false)[0];
    let instance = wgpu::Instance::new(desc.with_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&Default::default())).expect("a GPU adapter");
    let (device, queue) = request_device(&adapter).expect("device");
    let info = adapter.get_info();
    println!(
        "adapter {} ({:?}); device features {:?}; max_inter_stage_shader_variables {}",
        info.name,
        info.backend,
        device.features(),
        device.limits().max_inter_stage_shader_variables
    );
    Gpu {
        adapter,
        device,
        queue,
    }
}

fn bundled_preset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../third-party/slang-shaders")
        .join(config_layers::bundled_preset_rel_path(name).expect("bundled preset"))
}

/// Draws `frames` frames of a uniform `grey` core frame through `stage` into a
/// `size` target and returns the centre pixel (BGRA, gamma-encoded).
fn render(
    gpu: &Gpu,
    stage: &mut ShaderStage,
    size: (u32, u32),
    grey: u8,
    frames: usize,
) -> [u8; 4] {
    let quad = QuadPipeline::new(&gpu.device, TARGET_FORMAT, None);
    let frame_size = (256, 224);
    let tex = FrameTexture::new(
        &gpu.device,
        &quad,
        frame_size,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    );
    let pixels: Vec<u8> = (0..frame_size.0 * frame_size.1)
        .flat_map(|_| [grey, grey, grey, 255])
        .collect();
    tex.upload(&gpu.queue, &pixels);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_target"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[TARGET_FORMAT.remove_srgb_suffix()],
    });
    let surface_view = target.create_view(&Default::default());
    let chain_view = target.create_view(&wgpu::TextureViewDescriptor {
        format: Some(TARGET_FORMAT.remove_srgb_suffix()),
        ..Default::default()
    });
    let targets = StageTargets {
        surface_view: &surface_view,
        chain_view: &chain_view,
        chain_format: TARGET_FORMAT.remove_srgb_suffix(),
        size,
    };
    let rect = ViewportRect {
        x: 0.0,
        y: 0.0,
        width: size.0 as f32,
        height: size.1 as f32,
    };
    for _ in 0..frames {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        stage
            .record(&mut encoder, &targets, &quad, Some((&tex, rect)))
            .expect("frame");
        gpu.queue.submit([encoder.finish()]);
    }
    read_centre_pixel(gpu, &target, size)
}

fn read_centre_pixel(gpu: &Gpu, target: &wgpu::Texture, size: (u32, u32)) -> [u8; 4] {
    let row_bytes = size.0 * 4; // callers use widths that keep this 256-aligned
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row_bytes * size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(size.1),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    buffer.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = buffer.get_mapped_range(..).expect("mapped");
    let i = (((size.1 / 2) * size.0 + size.0 / 2) * 4) as usize;
    data[i..i + 4].try_into().expect("4 bytes")
}

fn chain_stage(gpu: &Gpu, preset: &Path) -> ShaderStage {
    let started = Instant::now();
    let chain = build_chain(preset, &gpu.device, &gpu.queue, gpu.adapter.get_info())
        .unwrap_or_else(|err| panic!("{}: {err}", preset.display()));
    println!(
        "{} built in {} ms",
        preset.display(),
        started.elapsed().as_millis()
    );
    ShaderStage::Chain {
        chain: Box::new(chain),
        preset: preset.to_path_buf(),
        frame_count: 0,
    }
}

#[test]
#[ignore = "needs a GPU adapter"]
fn bundled_presets_build_and_render_three_frames() {
    let gpu = gpu();
    for name in config_layers::BUNDLED_PRESETS {
        let mut stage = chain_stage(&gpu, &bundled_preset(name));
        let centre = render(&gpu, &mut stage, (1280, 960), 200, 3);
        println!("{name}: centre pixel BGRA {centre:?}");
        assert!(centre[..3].iter().any(|&c| c > 0), "{name} drew something");
    }
}

/// Gamma parity: a passthrough slang pass must reproduce the core's bytes exactly
/// like the plain quad does (input sampled as UNORM, output written through the
/// UNORM view). Sampling the sRGB view would darken 128 to about 55; writing
/// through the sRGB view would brighten it to about 188.
#[test]
#[ignore = "needs a GPU adapter"]
fn passthrough_chain_keeps_gamma_like_the_plain_quad() {
    let gpu = gpu();
    let preset = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-assets/passthrough.slangp");
    let plain = render(&gpu, &mut ShaderStage::Plain, (64, 64), 128, 1);
    let chained = render(&gpu, &mut chain_stage(&gpu, &preset), (64, 64), 128, 1);
    println!("plain {plain:?} chain {chained:?}");
    assert!(plain[0].abs_diff(128) <= 1, "plain quad {plain:?}");
    assert!(chained[0].abs_diff(128) <= 1, "chain {chained:?}");
}
