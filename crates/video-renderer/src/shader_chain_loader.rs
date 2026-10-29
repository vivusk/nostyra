//! Builds librashader filter chains on a worker thread: parsing a preset and
//! compiling its passes (glslang → SPIR-V → naga WGSL) takes tens to hundreds of
//! milliseconds, so the presenter keeps drawing the current stage meanwhile and
//! swaps the finished chain in when `poll` reports it.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use librashader_common::shader_features::ShaderFeatures;
use librashader_runtime_wgpu::error::FilterChainError;
use librashader_runtime_wgpu::options::FilterChainOptionsWgpu;
use librashader_runtime_wgpu::FilterChainWgpu;

/// A preset that could not be loaded or rendered; the plain picture stays up.
#[derive(Debug, thiserror::Error)]
#[error("shader preset {}: {message}", preset.display())]
pub struct ShaderLoadError {
    pub preset: PathBuf,
    pub message: String,
}

type BuildResult = Result<Box<FilterChainWgpu>, String>;

/// At most one build in flight; a newer request replaces (abandons) the older one.
#[derive(Default)]
pub(crate) struct ShaderChainLoader {
    pending: Option<(PathBuf, Receiver<BuildResult>)>,
}

impl ShaderChainLoader {
    /// Starts building `preset` on a new thread (abandoning any build in flight).
    pub fn start(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter: wgpu::AdapterInfo,
        preset: PathBuf,
    ) {
        let (sender, receiver) = mpsc::channel();
        let (device, queue, path) = (device.clone(), queue.clone(), preset.clone());
        let spawned = std::thread::Builder::new()
            .name("shader-chain-build".into())
            .spawn(move || {
                let started = Instant::now();
                // wgpu's default handler panics on validation errors; capture them
                // (the scope is per-thread) so a bad preset is an ordinary failure.
                let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let built = build_chain(&path, &device, &queue, adapter);
                let gpu_error = pollster::block_on(scope.pop());
                let result: BuildResult = match (built, gpu_error) {
                    (_, Some(err)) => Err(format!("GPU validation error: {err}")),
                    (Ok(chain), None) => Ok(Box::new(chain)),
                    (Err(err), None) => Err(err.to_string()),
                };
                tracing::info!(preset = %path.display(), ok = result.is_ok(),
                    elapsed_ms = started.elapsed().as_millis(), "shader chain build finished");
                // The receiver is gone when a newer request replaced this one.
                let _ = sender.send(result);
            });
        match spawned {
            Ok(_) => self.pending = Some((preset, receiver)),
            Err(err) => {
                // Report through `poll` like any other failure.
                let (sender, receiver) = mpsc::channel();
                let _ = sender.send(Err(format!("cannot start build thread: {err}")));
                self.pending = Some((preset, receiver));
            }
        }
    }

    /// Forgets the build in flight (its result is dropped on the worker thread).
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// The finished build, if any, with its preset path.
    pub fn poll(&mut self) -> Option<Result<(PathBuf, Box<FilterChainWgpu>), ShaderLoadError>> {
        let (preset, receiver) = self.pending.as_ref()?;
        let result = match receiver.try_recv() {
            Err(TryRecvError::Empty) => return None,
            Ok(Ok(chain)) => Ok((preset.clone(), chain)),
            Ok(Err(message)) => Err(ShaderLoadError {
                preset: preset.clone(),
                message,
            }),
            Err(TryRecvError::Disconnected) => Err(ShaderLoadError {
                preset: preset.clone(),
                message: "shader build thread panicked".into(),
            }),
        };
        self.pending = None;
        Some(result)
    }
}

/// Loads and compiles `preset` synchronously. The on-disk pipeline cache is only
/// usable when the device has `PIPELINE_CACHE` (wgpu: Vulkan); it lives in
/// librashader's own platform cache folder (not configurable in 0.12).
pub(crate) fn build_chain(
    preset: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    adapter: wgpu::AdapterInfo,
) -> Result<FilterChainWgpu, Box<FilterChainError>> {
    let options = FilterChainOptionsWgpu {
        force_no_mipmaps: false,
        enable_cache: device.features().contains(wgpu::Features::PIPELINE_CACHE),
        adapter_info: Some(adapter),
    };
    FilterChainWgpu::load_from_path(preset, ShaderFeatures::NONE, device, queue, Some(&options))
        .map_err(Box::new)
}
