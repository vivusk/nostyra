//! After a core geometry change (`take_av_change`): the picture takes the new display
//! aspect at once. When the aspect really changed (melonDS DS R3 layout swap:
//! 256×384 ↔ 512×192) a normal window is resized by the start-up window rule
//! (`runner_window::window_size_for`), so the picture is not letterboxed to a strip;
//! fullscreen and maximized windows only re-fit the picture. An unchanged aspect
//! (melonDS DS repeats 256×384 on its first frame) leaves the window alone.
//! Best effort: `request_inner_size` may apply later or not at all (macOS, tiling
//! window managers); the picture fit is right either way. The touch rect follows on
//! the next redraw (`pointer_events` reads the current geometry).

use libretro_host::RetroGameGeometry;
use winit::dpi::LogicalSize;

use super::EmulationApp;
use crate::runner_window::{display_aspect, monitor_logical_size, window_size_for};

/// Relative aspect difference below which the layout counts as unchanged.
const SAME_ASPECT: f64 = 1e-3;

/// New logical inner size after a geometry change, or `None` to keep the window:
/// unknown (`<= 0`) or unchanged previous aspect, fullscreen or maximized window.
pub(crate) fn resize_target(
    previous_aspect: f32,
    geometry: &RetroGameGeometry,
    fullscreen: bool,
    maximized: bool,
    monitor: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let previous = f64::from(previous_aspect);
    if previous <= 0.0 || fullscreen || maximized {
        return None;
    }
    let aspect = display_aspect(geometry);
    if ((aspect - previous) / previous).abs() < SAME_ASPECT {
        return None;
    }
    Some(window_size_for(geometry, monitor))
}

impl EmulationApp<'_> {
    /// Applies the core's new geometry to the picture and, per [`resize_target`],
    /// to the window.
    pub(super) fn apply_geometry(&mut self, geometry: RetroGameGeometry) {
        let Some(presenter) = self.presenter.as_mut() else {
            return;
        };
        let previous = presenter.aspect_ratio();
        presenter.set_aspect_ratio(geometry.aspect_ratio);
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let monitor = monitor_logical_size(window.current_monitor());
        let fullscreen = window.fullscreen().is_some();
        if let Some((width, height)) = resize_target(
            previous,
            &geometry,
            fullscreen,
            window.is_maximized(),
            monitor,
        ) {
            tracing::info!(width, height, "core layout changed; resizing the window");
            let _ = window.request_inner_size(LogicalSize::new(width, height));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(width: u32, height: u32) -> RetroGameGeometry {
        RetroGameGeometry {
            base_width: width,
            base_height: height,
            max_width: 1024,
            max_height: 576,
            aspect_ratio: width as f32 / height as f32,
        }
    }

    const TALL: f32 = 256.0 / 384.0;
    const MONITOR: Option<(f64, f64)> = Some((1920.0, 1080.0));

    #[test]
    fn layout_swap_resizes_a_normal_window_by_the_start_rule() {
        let wide = geometry(512, 192);
        let (w, h) = resize_target(TALL, &wide, false, false, MONITOR).expect("resize");
        // 3× 192 = 576 high at 8:3 → 1536 × 576 (fits 90 % of 1920 × 1080).
        assert!((w - 1536.0).abs() < 1e-3 && h == 576.0, "{w}x{h}");
        let back = resize_target(512.0 / 192.0, &geometry(256, 384), false, false, MONITOR);
        let (w, h) = back.expect("resize back");
        assert!(
            (w - 480.0).abs() < 1e-3 && h == 720.0,
            "start-up size: {w}x{h}"
        );
    }

    #[test]
    fn unchanged_aspect_keeps_the_window() {
        // melonDS DS repeats 256×384 on its first frame.
        let same = geometry(256, 384);
        assert_eq!(resize_target(TALL, &same, false, false, MONITOR), None);
        // Same aspect at another size (resolution switch) is not a layout change.
        assert_eq!(
            resize_target(TALL, &geometry(512, 768), false, false, None),
            None
        );
    }

    #[test]
    fn fullscreen_maximized_or_unknown_aspect_only_refit() {
        let wide = geometry(512, 192);
        assert_eq!(resize_target(TALL, &wide, true, false, MONITOR), None);
        assert_eq!(resize_target(TALL, &wide, false, true, MONITOR), None);
        assert_eq!(resize_target(0.0, &wide, false, false, MONITOR), None);
    }
}
