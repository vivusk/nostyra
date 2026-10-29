//! Output/input traits the core's callbacks forward into, plus headless null impls.
//!
//! Sinks are borrowed only for the duration of one `run_frame`, so implementors
//! never deal with C pointers or threads. Callbacks are hot paths: implementations
//! must not allocate per call.

/// Pixel layout of frames produced by the core (`RETRO_PIXEL_FORMAT_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// libretro default; 15-bit color in 16-bit words.
    Rgb1555,
    /// 32-bit words, top byte unused.
    Xrgb8888,
    /// 16-bit words; Snes9x's choice.
    Rgb565,
    /// Bytes R, G, B, A: frames read back from a hardware framebuffer (alpha ignored).
    Rgba8888,
}

impl PixelFormat {
    /// Bytes per pixel in the core's frame buffer.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Xrgb8888 | Self::Rgba8888 => 4,
            Self::Rgb1555 | Self::Rgb565 => 2,
        }
    }
}

/// One frame as handed over by `retro_video_refresh`. Valid only inside the callback.
#[derive(Clone, Copy, Debug)]
pub struct VideoFrame<'a> {
    /// Raw rows; `height * pitch` bytes (last row may be shorter than `pitch`).
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Bytes between row starts (may exceed `width * bytes_per_pixel`).
    pub pitch: usize,
    pub format: PixelFormat,
    /// Rows are stored bottom row first (OpenGL readback); [`Self::row`] hides it.
    pub bottom_up: bool,
}

impl VideoFrame<'_> {
    /// Visible bytes of display row `y` (0 = top), excluding pitch padding.
    pub fn row(&self, y: u32) -> &[u8] {
        let stored = if self.bottom_up {
            self.height - 1 - y
        } else {
            y
        };
        let start = stored as usize * self.pitch;
        &self.data[start..start + self.width as usize * self.format.bytes_per_pixel()]
    }
}

/// Receives video frames from the core.
pub trait VideoSink {
    /// A new frame was rendered.
    fn frame(&mut self, frame: &VideoFrame<'_>);
    /// Core passed NULL ("dupe"): show the previous frame again.
    fn dupe(&mut self, width: u32, height: u32);
}

/// Receives interleaved stereo `i16` samples (L, R, L, R, ...).
pub trait AudioSink {
    fn samples(&mut self, interleaved: &[i16]);
}

/// Answers the core's input queries.
pub trait InputSource {
    /// Called once per frame when the core polls input; refresh device state here.
    fn poll(&mut self);
    /// State of one button/axis (`RETRO_DEVICE_*` / `RETRO_DEVICE_ID_*`).
    fn state(&mut self, port: u32, device: u32, index: u32, id: u32) -> i16;
}

/// Headless video sink: counts frames and keeps a checksum of the last frame
/// (FNV-1a over visible pixels, rows in display order) for determinism tests.
#[derive(Debug, Default)]
pub struct NullVideoSink {
    pub frames: u64,
    pub dupes: u64,
    pub last_checksum: u64,
    pub last_size: (u32, u32),
}

impl VideoSink for NullVideoSink {
    fn frame(&mut self, frame: &VideoFrame<'_>) {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for y in 0..frame.height {
            for &byte in frame.row(y) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        self.frames += 1;
        self.last_checksum = hash;
        self.last_size = (frame.width, frame.height);
    }

    fn dupe(&mut self, width: u32, height: u32) {
        self.dupes += 1;
        self.last_size = (width, height);
    }
}

/// Headless audio sink: counts stereo frames and tracks the peak amplitude.
#[derive(Debug, Default)]
pub struct NullAudioSink {
    pub stereo_frames: u64,
    pub peak: u16,
}

impl AudioSink for NullAudioSink {
    fn samples(&mut self, interleaved: &[i16]) {
        self.stereo_frames += (interleaved.len() / 2) as u64;
        for &s in interleaved {
            self.peak = self.peak.max(s.unsigned_abs());
        }
    }
}

/// Headless input: nothing is ever pressed.
#[derive(Debug, Default)]
pub struct NullInputSource;

impl InputSource for NullInputSource {
    fn poll(&mut self) {}

    fn state(&mut self, _port: u32, _device: u32, _index: u32, _id: u32) -> i16 {
        0
    }
}
