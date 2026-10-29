//! `VideoSink` that keeps the latest core frame as RGBA8 on the CPU.
//!
//! Decoupled from the GPU presenter so conversion is testable headless and the
//! libretro callback stays cheap (no GPU calls inside `retro_run`).

use libretro_host::{VideoFrame, VideoSink};

use crate::pixel_format_converter::convert_to_rgba;

/// Latest frame in RGBA8, plus a dirty flag so unchanged frames aren't re-uploaded.
#[derive(Debug, Default)]
pub struct RgbaFrameBuffer {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    dirty: bool,
}

impl RgbaFrameBuffer {
    /// Tightly packed RGBA8 rows of the latest frame.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Size of the latest frame; `(0, 0)` before the first frame.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns true once per new frame (consumed by the presenter upload).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

impl VideoSink for RgbaFrameBuffer {
    fn frame(&mut self, frame: &VideoFrame<'_>) {
        convert_to_rgba(frame, &mut self.pixels);
        self.width = frame.width;
        self.height = frame.height;
        self.dirty = true;
    }

    /// Dupe: keep the previous pixels; the presenter simply redraws them.
    fn dupe(&mut self, _width: u32, _height: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretro_host::PixelFormat;

    #[test]
    fn dupe_keeps_previous_frame_and_is_not_dirty() {
        let mut buf = RgbaFrameBuffer::default();
        let data = [0xFF, 0xFF];
        buf.frame(&VideoFrame {
            data: &data,
            width: 1,
            height: 1,
            pitch: 2,
            format: PixelFormat::Rgb565,
            bottom_up: false,
        });
        assert!(buf.take_dirty());
        buf.dupe(1, 1);
        assert!(!buf.take_dirty());
        assert_eq!(buf.pixels(), [255, 255, 255, 255]);
        assert_eq!(buf.size(), (1, 1));
    }
}
