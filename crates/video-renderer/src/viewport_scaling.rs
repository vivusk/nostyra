//! Where the game image goes inside the window, per scaling mode. Pure math so
//! it is unit-testable without a GPU.

/// How the frame is fitted into the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScalingMode {
    /// Largest size with the core's display aspect (SNES 8:7 PAR → 4:3).
    #[default]
    AspectFit,
    /// Like `AspectFit` but frame height is an integer multiple (sharp scanlines);
    /// falls back to `AspectFit` when the window is smaller than 1×.
    Integer,
    /// Fill the whole window, ignoring aspect.
    Stretch,
}

/// Viewport rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Computes the viewport for `frame` (w, h) inside `window` (w, h).
///
/// `aspect` is the core's display aspect ratio; `<= 0` means "use w/h"
/// (libretro convention).
pub fn compute_viewport(
    window: (u32, u32),
    frame: (u32, u32),
    aspect: f32,
    mode: ScalingMode,
) -> ViewportRect {
    let (ww, wh) = (window.0 as f32, window.1 as f32);
    let (fw, fh) = (frame.0.max(1) as f32, frame.1.max(1) as f32);
    let aspect = if aspect > 0.0 { aspect } else { fw / fh };

    let (width, height) = match mode {
        ScalingMode::Stretch => (ww, wh),
        ScalingMode::AspectFit => fit(ww, wh, aspect),
        ScalingMode::Integer => {
            let scale = (wh / fh).min(ww / (fh * aspect)).floor();
            if scale >= 1.0 {
                let height = fh * scale;
                ((height * aspect).round(), height)
            } else {
                fit(ww, wh, aspect)
            }
        }
    };
    ViewportRect {
        x: ((ww - width) / 2.0).floor(),
        y: ((wh - height) / 2.0).floor(),
        width,
        height,
    }
}

fn fit(ww: f32, wh: f32, aspect: f32) -> (f32, f32) {
    if ww / wh > aspect {
        ((wh * aspect).round(), wh)
    } else {
        (ww, (ww / aspect).round())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNES: (u32, u32) = (256, 224);
    const FOUR_THIRDS: f32 = 4.0 / 3.0;

    #[test]
    fn aspect_fit_pillarboxes_wide_window() {
        let r = compute_viewport((1920, 1080), SNES, FOUR_THIRDS, ScalingMode::AspectFit);
        assert_eq!(
            r,
            ViewportRect {
                x: 240.0,
                y: 0.0,
                width: 1440.0,
                height: 1080.0
            }
        );
    }

    #[test]
    fn aspect_fit_letterboxes_tall_window() {
        let r = compute_viewport((800, 1000), SNES, FOUR_THIRDS, ScalingMode::AspectFit);
        assert_eq!((r.width, r.height, r.y), (800.0, 600.0, 200.0));
    }

    #[test]
    fn integer_uses_whole_multiples_of_frame_height() {
        // 1080 / 224 = 4.82 → 4× = 896 lines, width 896 × 4/3 ≈ 1195.
        let r = compute_viewport((1920, 1080), SNES, FOUR_THIRDS, ScalingMode::Integer);
        assert_eq!((r.width, r.height), (1195.0, 896.0));
        assert_eq!((r.x, r.y), (362.0, 92.0));
    }

    #[test]
    fn integer_falls_back_when_window_too_small() {
        let r = compute_viewport((200, 150), SNES, FOUR_THIRDS, ScalingMode::Integer);
        assert_eq!((r.width, r.height), (200.0, 150.0));
    }

    #[test]
    fn stretch_and_unknown_aspect() {
        let r = compute_viewport((640, 480), SNES, 0.0, ScalingMode::Stretch);
        assert_eq!((r.width, r.height), (640.0, 480.0));
        // aspect 0 → frame's own 256:224 ratio.
        let r = compute_viewport((512, 1000), SNES, 0.0, ScalingMode::AspectFit);
        assert_eq!((r.width, r.height), (512.0, 448.0));
    }
}
