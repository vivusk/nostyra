//! Test doubles for the hardware-render API: a provider that accepts OpenGL core
//! ≤ 3.3 and a context that records every call into a shared log.

use std::ffi::{c_void, CStr};
use std::sync::atomic::AtomicUsize;
use std::sync::{Mutex, MutexGuard};

use crate::frame_sink_traits::{PixelFormat, VideoFrame, VideoSink};
use crate::hw_render::{HwContextKind, HwRenderContext, HwRenderProvider, HwRenderRequest};

/// Calls made by the fakes (and by fake core callbacks in tests), in order.
pub(crate) static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Tests sharing `LOG` (and the thread-local context) run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());
/// The fake core's `retro_serialize_size` (reset to 4 by [`serial`]).
pub(crate) static FAKE_STATE_SIZE: AtomicUsize = AtomicUsize::new(4);

pub(crate) fn serial() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    log_take();
    FAKE_STATE_SIZE.store(4, std::sync::atomic::Ordering::SeqCst);
    guard
}

pub(crate) fn log(entry: impl Into<String>) {
    LOG.lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(entry.into());
}

pub(crate) fn log_take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap_or_else(|e| e.into_inner()))
}

pub(crate) const FAKE_FBO: usize = 7;
pub(crate) const FAKE_PROC: usize = 0x1234;

#[derive(Debug, Default)]
pub(crate) struct FakeProvider {
    pub fail_create: bool,
}

impl HwRenderProvider for FakeProvider {
    fn supports(&self, request: &HwRenderRequest) -> bool {
        matches!(request.kind, HwContextKind::OpenGlCore { major: 3, minor } if minor <= 3)
    }

    fn preferred(&self) -> HwContextKind {
        HwContextKind::OpenGlCore { major: 3, minor: 3 }
    }

    fn create(
        &self,
        _request: &HwRenderRequest,
        max_width: u32,
        max_height: u32,
    ) -> Result<Box<dyn HwRenderContext>, String> {
        log(format!("create {max_width}x{max_height}"));
        if self.fail_create {
            return Err("no OpenGL 3.3".into());
        }
        Ok(Box::new(FakeContext {
            size: (max_width, max_height),
        }))
    }
}

pub(crate) struct FakeContext {
    pub size: (u32, u32),
}

impl HwRenderContext for FakeContext {
    fn make_current(&mut self) -> Result<(), String> {
        log("current");
        Ok(())
    }

    fn framebuffer_id(&self) -> usize {
        FAKE_FBO
    }

    fn proc_address(&self, name: &CStr) -> *const c_void {
        log(format!("proc {}", name.to_string_lossy()));
        FAKE_PROC as *const c_void
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        log(format!("resize {width}x{height}"));
        self.size = (width, height);
        Ok(())
    }

    /// Delivers a bottom-up 1×2 frame: stored row 0 blue (bottom), row 1 red (top).
    fn deliver_frame(&mut self, width: u32, height: u32, sink: &mut dyn VideoSink) {
        log(format!("deliver {width}x{height}"));
        let data = [0, 0, 255, 0, 255, 0, 0, 0];
        sink.frame(&VideoFrame {
            data: &data,
            width: 1,
            height: 2,
            pitch: 4,
            format: PixelFormat::Rgba8888,
            bottom_up: true,
        });
    }
}

impl Drop for FakeContext {
    fn drop(&mut self) {
        log("drop context");
    }
}
