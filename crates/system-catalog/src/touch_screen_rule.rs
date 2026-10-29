//! Where a system's touch screen (or aiming pointer) sits inside the core's output
//! frame. Cores with a pointer report coordinates over the whole frame; presses
//! outside the touch screen must not reach them (melonDS DS clamps a press on the
//! top screen to the bottom screen's edge), so the runner asks [`touch_rect`] for
//! the current geometry before reporting a press. An aiming rule
//! ([`TouchScreenRule::aims`]) also changes how the runner treats the mouse: the
//! pointer parks at the picture edge instead of vanishing, and the OS cursor hides
//! over the picture (the game draws its own).

/// The touch screen layout rule of a system (`SystemSpec.touch_screen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchScreenRule {
    /// Nintendo DS: two 256×192 screens composited by the core. Top-bottom
    /// (aspect 2:3) → the bottom half; left-right (8:3) → the right half; any other
    /// layout → the whole frame.
    DsBottomScreen,
    /// Nintendo Wii: the Wii Remote's IR pointer aimed with the mouse; the whole
    /// frame is the pointer area (Dolphin `ir_mode` 2 reads X/Y only).
    WiiRemoteIr,
}

impl TouchScreenRule {
    /// An aiming pointer rather than a touch screen: off the picture the pointer
    /// keeps its position clamped to the picture edge (the core ignores
    /// `IS_OFFSCREEN`), and the OS cursor is hidden over the picture.
    pub fn aims(self) -> bool {
        matches!(self, Self::WiiRemoteIr)
    }
}

/// Part of the output frame, in fractions of its width / height (0..=1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl TouchRect {
    /// The whole frame.
    pub const FULL: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };

    /// `(x, y)` in frame fractions lies inside (edges included).
    pub fn contains(&self, x: f32, y: f32) -> bool {
        (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y)
    }
}

/// The touch rect of `rule` for a frame of `width` × `height` (the core's base
/// geometry). Exact integer ratios, so scaled renderers (512×768) match too; a zero
/// size gives the whole frame.
pub fn touch_rect(rule: TouchScreenRule, width: u32, height: u32) -> TouchRect {
    let (w, h) = (u64::from(width), u64::from(height));
    match rule {
        TouchScreenRule::DsBottomScreen if w > 0 && w * 3 == h * 2 => TouchRect {
            top: 0.5,
            ..TouchRect::FULL
        },
        TouchScreenRule::DsBottomScreen if h > 0 && w * 3 == h * 8 => TouchRect {
            left: 0.5,
            ..TouchRect::FULL
        },
        TouchScreenRule::DsBottomScreen | TouchScreenRule::WiiRemoteIr => TouchRect::FULL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DS: TouchScreenRule = TouchScreenRule::DsBottomScreen;

    #[test]
    fn top_bottom_layout_touches_the_bottom_half() {
        for (w, h) in [(256, 384), (512, 768), (1024, 1536)] {
            let rect = touch_rect(DS, w, h);
            assert_eq!(rect.top, 0.5, "{w}x{h}");
            assert_eq!((rect.left, rect.right, rect.bottom), (0.0, 1.0, 1.0));
            assert!(rect.contains(0.5, 0.75));
            assert!(!rect.contains(0.5, 0.25), "top screen");
        }
    }

    #[test]
    fn left_right_layout_touches_the_right_half() {
        for (w, h) in [(512, 192), (1024, 384)] {
            let rect = touch_rect(DS, w, h);
            assert_eq!(rect.left, 0.5, "{w}x{h}");
            assert_eq!((rect.top, rect.right, rect.bottom), (0.0, 1.0, 1.0));
            assert!(rect.contains(0.75, 0.5));
            assert!(!rect.contains(0.25, 0.5), "left (top) screen");
        }
    }

    #[test]
    fn other_ratios_and_zero_sizes_touch_the_whole_frame() {
        for (w, h) in [
            (256, 192),
            (256, 385),
            (384, 256),
            (0, 0),
            (0, 384),
            (256, 0),
        ] {
            assert_eq!(touch_rect(DS, w, h), TouchRect::FULL, "{w}x{h}");
        }
        assert!(TouchRect::FULL.contains(0.0, 1.0), "edges included");
        assert!(!TouchRect::FULL.contains(1.01, 0.5));
    }

    #[test]
    fn wii_ir_aims_over_the_whole_frame_at_any_geometry() {
        let wii = TouchScreenRule::WiiRemoteIr;
        // 640×528 EFB 1×, 4:3 / 16:9 picture sizes, the DS ratios, zero sizes.
        for (w, h) in [
            (640, 528),
            (640, 480),
            (854, 480),
            (256, 384),
            (512, 192),
            (0, 0),
        ] {
            assert_eq!(touch_rect(wii, w, h), TouchRect::FULL, "{w}x{h}");
        }
        assert!(wii.aims());
        assert!(!DS.aims(), "the DS touch screen is no aiming pointer");
    }
}
