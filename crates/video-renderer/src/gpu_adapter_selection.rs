//! Picks the GPU backend + adapter for a window.
//!
//! Preference: DX12 on Windows (native flip-model presentation, best frame
//! pacing), otherwise wgpu's primary backends (Metal on macOS, Vulkan on Linux).
//! If the preferred set yields no adapter, falls back to all primary backends.
//! `WGPU_BACKEND` (e.g. `vulkan`, `dx12`, `metal`, `gl`) overrides for debugging,
//! except a forced `gl` while a core renders through its own OpenGL context (two GL
//! users on one thread would fight over the current context): primary backends then.
//! The device asks only for the optional features librashader can use that the
//! adapter actually offers, so a missing one never fails device creation.

use std::sync::Arc;

use winit::window::Window;

use crate::renderer_error::RendererError;

/// Ordered backend sets to try for this platform/environment. `gl_in_use`: a core
/// renders through OpenGL in this process.
pub(crate) fn candidate_backends(gl_in_use: bool) -> Vec<wgpu::Backends> {
    backends_for(wgpu::Backends::from_env(), gl_in_use)
}

/// [`candidate_backends`] for a `WGPU_BACKEND` value (`forced`).
fn backends_for(forced: Option<wgpu::Backends>, gl_in_use: bool) -> Vec<wgpu::Backends> {
    match forced {
        Some(forced) if gl_in_use && forced.contains(wgpu::Backends::GL) => {
            tracing::warn!(?forced, "WGPU_BACKEND with gl ignored: a core uses OpenGL");
            return vec![wgpu::Backends::PRIMARY];
        }
        Some(forced) => return vec![forced],
        None => {}
    }
    if cfg!(windows) {
        vec![wgpu::Backends::DX12, wgpu::Backends::PRIMARY]
    } else {
        vec![wgpu::Backends::PRIMARY]
    }
}

/// Creates a surface for `window` and an adapter able to present to it.
pub(crate) fn open_surface_and_adapter(
    window: &Arc<Window>,
    gl_in_use: bool,
) -> Result<(wgpu::Surface<'static>, wgpu::Adapter), RendererError> {
    let mut last_err = None;
    for backends in candidate_backends(gl_in_use) {
        let desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()));
        // `with_env` would re-apply `WGPU_BACKEND`: `backends` already decided it.
        let mut desc = desc.with_env();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone())?;
        let request = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        });
        match pollster::block_on(request) {
            Ok(adapter) => return Ok((surface, adapter)),
            Err(err) => {
                tracing::warn!(?backends, %err, "no adapter for backend set; trying next");
                last_err = Some(err);
            }
        }
    }
    // `candidate_backends` is never empty, so `last_err` is always set here.
    Err(last_err.map_or(RendererError::NoAdapter, RendererError::RequestAdapter))
}

/// Optional features librashader uses when present: border clamp (else clamp to
/// edge), pipeline cache (Vulkan only; enables its on-disk cache), filterable f32
/// framebuffers and adapter-specific format capabilities (e.g. sRGB render targets).
const SHADER_FEATURES: wgpu::Features = wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
    .union(wgpu::Features::PIPELINE_CACHE)
    .union(wgpu::Features::FLOAT32_FILTERABLE)
    .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);

/// Opens the device with the shader features the adapter supports and its full
/// inter-stage variable budget (multi-output CRT passes exceed wgpu's default).
pub(crate) fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), RendererError> {
    let features = adapter.features() & SHADER_FEATURES;
    let limits = wgpu::Limits {
        max_inter_stage_shader_variables: adapter.limits().max_inter_stage_shader_variables,
        ..wgpu::Limits::default()
    };
    tracing::debug!(
        ?features,
        limits.max_inter_stage_shader_variables,
        "device request"
    );
    let desc = wgpu::DeviceDescriptor {
        label: Some("nostyra"),
        required_features: features,
        required_limits: limits,
        ..Default::default()
    };
    Ok(pollster::block_on(adapter.request_device(&desc))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_gl_backend_is_refused_while_a_core_uses_opengl() {
        let gl = Some(wgpu::Backends::GL);
        assert_eq!(backends_for(gl, false), vec![wgpu::Backends::GL]);
        assert_eq!(backends_for(gl, true), vec![wgpu::Backends::PRIMARY]);
        let mixed = Some(wgpu::Backends::GL | wgpu::Backends::VULKAN);
        assert_eq!(backends_for(mixed, true), vec![wgpu::Backends::PRIMARY]);
        let vulkan = Some(wgpu::Backends::VULKAN);
        assert_eq!(backends_for(vulkan, true), vec![wgpu::Backends::VULKAN]);
        let default = backends_for(None, true);
        assert_eq!(default.last(), Some(&wgpu::Backends::PRIMARY));
        assert!(default.iter().all(|b| !b.contains(wgpu::Backends::GL)));
    }
}
