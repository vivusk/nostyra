//! Error and outcome types of the GPU presenter.

/// Failures while setting up or driving the GPU.
#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("cannot create window surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible GPU adapter: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    #[error("no GPU backend candidates")]
    NoAdapter,
    #[error("cannot open GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("surface reports no supported texture formats")]
    NoSurfaceFormat,
    #[error("surface validation error while acquiring a frame")]
    SurfaceValidation,
}

/// Result of one `render` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentOutcome {
    Presented,
    /// Window occluded/minimized, surface reconfigured or timed out; try next tick.
    Skipped,
}
