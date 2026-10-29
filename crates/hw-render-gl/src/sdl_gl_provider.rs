//! [`SdlGlProvider`]: answers `SET_HW_RENDER` for OpenGL and builds the context.
//!
//! `supports` / `preferred` run inside `retro_load_game` and touch nothing. SDL (and
//! its video subsystem) is initialised lazily in `create`, so software cores never
//! start SDL video or create a GL context.

use std::cell::OnceCell;

use libretro_host::{HwContextKind, HwRenderContext, HwRenderProvider, HwRenderRequest};
use sdl3::Sdl;

use crate::gl_offscreen_context::GlOffscreenContext;

/// Highest core-profile version we promise (GLideN64 needs 3.3; macOS gives 4.1).
const MAX_CORE: (u32, u32) = (3, 3);

/// OpenGL [`HwRenderProvider`] backed by SDL3. Lives on the emulation thread.
#[derive(Default)]
pub struct SdlGlProvider {
    sdl: OnceCell<Sdl>,
}

impl std::fmt::Debug for SdlGlProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdlGlProvider")
            .field("video_initialised", &self.video_initialised())
            .finish()
    }
}

impl SdlGlProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// True once `create` initialised SDL (i.e. a core asked for OpenGL).
    pub fn video_initialised(&self) -> bool {
        self.sdl.get().is_some()
    }

    /// The SDL handle, initialised on first use (`get` + `set`: `get_or_try_init` is
    /// not stable on the pinned toolchain).
    fn sdl(&self) -> Result<&Sdl, String> {
        if self.sdl.get().is_none() {
            let sdl = sdl3::init().map_err(|e| format!("SDL init failed: {e}"))?;
            let _ = self.sdl.set(sdl);
        }
        self.sdl.get().ok_or_else(|| "SDL init failed".to_owned())
    }
}

/// True for OpenGL requests this backend can serve.
pub(crate) fn supported(kind: HwContextKind) -> bool {
    match kind {
        HwContextKind::OpenGl => true,
        HwContextKind::OpenGlCore { major, minor } => (major, minor) <= MAX_CORE,
        _ => false,
    }
}

impl HwRenderProvider for SdlGlProvider {
    fn supports(&self, request: &HwRenderRequest) -> bool {
        supported(request.kind)
    }

    fn preferred(&self) -> HwContextKind {
        HwContextKind::OpenGlCore {
            major: MAX_CORE.0,
            minor: MAX_CORE.1,
        }
    }

    fn create(
        &self,
        request: &HwRenderRequest,
        max_width: u32,
        max_height: u32,
    ) -> Result<Box<dyn HwRenderContext>, String> {
        if !supported(request.kind) {
            return Err(format!("unsupported graphics API {:?}", request.kind));
        }
        let video = self
            .sdl()?
            .video()
            .map_err(|e| format!("SDL video init failed: {e}"))?;
        let context = GlOffscreenContext::new(video, request, max_width, max_height)?;
        Ok(Box::new(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_gl_core_up_to_33_and_compat_only() {
        assert!(supported(HwContextKind::OpenGlCore { major: 3, minor: 3 }));
        assert!(supported(HwContextKind::OpenGlCore { major: 3, minor: 1 }));
        assert!(supported(HwContextKind::OpenGl));
        assert!(!supported(HwContextKind::OpenGlCore { major: 4, minor: 5 }));
        assert!(!supported(HwContextKind::Vulkan));
        assert!(!supported(HwContextKind::OpenGlEs3));
    }

    #[test]
    fn supports_and_preferred_never_initialise_sdl() {
        let provider = SdlGlProvider::new();
        let request = HwRenderRequest {
            kind: HwContextKind::OpenGlCore { major: 3, minor: 3 },
            depth: true,
            stencil: false,
            bottom_left_origin: true,
            cache_context: true,
            debug: false,
        };
        assert!(provider.supports(&request));
        let _ = provider.preferred();
        assert!(!provider.video_initialised());
    }
}
