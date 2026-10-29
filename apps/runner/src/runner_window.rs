//! Window helpers: size from the core's geometry (at start and after a layout
//! change, `window_resize_on_geometry`), display refresh rate, fullscreen toggle.

use std::sync::Arc;

use libretro_host::{AvInfo, RetroGameGeometry};
use video_renderer::{GpuFramePresenter, PresentMode};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window};

/// Default window height in multiples of the core's base height.
const INITIAL_SCALE: f64 = 3.0;
/// Tallest 3× window; cores with a larger base (HD-rendering GPU cores, e.g. N64 at
/// 960×720) open at [`HD_CORE_HEIGHT`] instead.
const MAX_SCALED_HEIGHT: f64 = 1080.0;
const HD_CORE_HEIGHT: f64 = 720.0;
/// The window never exceeds this share of the primary monitor.
const MONITOR_SHARE: f64 = 0.9;

/// Initial logical inner size: 3× base height (720 when that exceeds 1080) at
/// `aspect`, scaled down to fit 90 % of `monitor` (logical width, height) if known.
fn initial_inner_size(base_height: f64, aspect: f64, monitor: Option<(f64, f64)>) -> (f64, f64) {
    let scaled = base_height * INITIAL_SCALE;
    let mut height = if scaled <= MAX_SCALED_HEIGHT {
        scaled
    } else {
        HD_CORE_HEIGHT
    };
    if let Some((monitor_w, monitor_h)) = monitor {
        let fit = (monitor_h * MONITOR_SHARE).min(monitor_w * MONITOR_SHARE / aspect);
        if fit > 0.0 {
            height = height.min(fit);
        }
    }
    (height * aspect, height)
}

/// Display aspect of `geometry`: the core's, else base width / height.
pub fn display_aspect(geometry: &RetroGameGeometry) -> f64 {
    if geometry.aspect_ratio > 0.0 {
        f64::from(geometry.aspect_ratio)
    } else {
        f64::from(geometry.base_width) / f64::from(geometry.base_height.max(1))
    }
}

/// Logical (width, height) of `monitor`, if known.
pub fn monitor_logical_size(monitor: Option<MonitorHandle>) -> Option<(f64, f64)> {
    monitor.map(|m| {
        let size = m.size().to_logical::<f64>(m.scale_factor());
        (size.width, size.height)
    })
}

/// Logical inner size for `geometry` on `monitor` ([`initial_inner_size`]).
pub fn window_size_for(geometry: &RetroGameGeometry, monitor: Option<(f64, f64)>) -> (f64, f64) {
    let base_height = f64::from(geometry.base_height.max(1));
    initial_inner_size(base_height, display_aspect(geometry), monitor)
}

/// Creates the game window ([`window_size_for`]) with the core's display aspect.
fn create_game_window(event_loop: &ActiveEventLoop, av: &AvInfo) -> anyhow::Result<Arc<Window>> {
    let monitor = monitor_logical_size(event_loop.primary_monitor());
    let (width, height) = window_size_for(&av.geometry, monitor);
    let attrs = crate::runner_window_icon::with_logo_icon(
        Window::default_attributes()
            .with_title("Nostyra")
            .with_inner_size(LogicalSize::new(width, height)),
    );
    Ok(Arc::new(event_loop.create_window(attrs)?))
}

/// Creates the window plus its GPU presenter (Fifo until the sync mode is chosen)
/// and requests the first redraw. `gl_in_use`: the core renders through OpenGL.
pub fn open_game_window(
    event_loop: &ActiveEventLoop,
    av: &AvInfo,
    fullscreen: bool,
    gl_in_use: bool,
) -> anyhow::Result<(Arc<Window>, GpuFramePresenter)> {
    let window = create_game_window(event_loop, av)?;
    let mut presenter = GpuFramePresenter::new(window.clone(), PresentMode::Fifo, gl_in_use)?;
    presenter.set_aspect_ratio(av.geometry.aspect_ratio);
    if fullscreen {
        toggle_fullscreen(&window);
    }
    window.request_redraw();
    Ok((window, presenter))
}

/// Refresh rate (Hz) of the monitor the window is on; `None` if unknown.
pub fn display_hz(window: &Window) -> Option<f64> {
    window
        .current_monitor()
        .and_then(|m| m.refresh_rate_millihertz())
        .map(|mhz| f64::from(mhz) / 1000.0)
}

pub fn toggle_fullscreen(window: &Window) {
    let next = match window.fullscreen() {
        Some(_) => None,
        None => Some(Fullscreen::Borderless(None)),
    };
    window.set_fullscreen(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_size_is_3x_base_or_720_for_hd_cores() {
        let size = |w: f64, h: f64| initial_inner_size(h, w / h, None);
        assert_eq!(size(256.0, 224.0).1, 672.0, "SNES");
        assert_eq!(size(240.0, 160.0).1, 480.0, "GBA");
        assert_eq!(size(960.0, 720.0), (960.0, 720.0), "N64 at 3x resolution");
        assert_eq!(size(640.0, 480.0), (960.0, 720.0), "N64 native");
        assert_eq!(size(320.0, 360.0).1, 1080.0, "exactly 1080 stays 3x");
    }

    #[test]
    fn initial_size_fits_90_percent_of_the_monitor() {
        let (w, h) = initial_inner_size(224.0, 4.0 / 3.0, Some((1280.0, 720.0)));
        assert!(
            (h - 648.0).abs() < 1e-9 && (w - 864.0).abs() < 1e-9,
            "{w}x{h}"
        );
        let (w, h) = initial_inner_size(720.0, 16.0 / 9.0, Some((1000.0, 1200.0)));
        assert!(
            (w - 900.0).abs() < 1e-9 && h < 720.0,
            "{w}x{h} (width-bound)"
        );
        let big = initial_inner_size(224.0, 4.0 / 3.0, Some((3840.0, 2160.0)));
        assert_eq!(big.1, 672.0, "no upscaling on a large monitor");
    }
}
