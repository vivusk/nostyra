//! Graphics contexts for cores that render on the GPU (`SET_HW_RENDER`).
//!
//! One [`HwRenderSetup`] per run, built before `CoreSession::load` and handed to it
//! through `SessionConfig::hw_render`. It wraps the OpenGL backend
//! ([`hw_render_gl::SdlGlProvider`]), which initialises SDL video only inside
//! `create`, i.e. only when a core asked for OpenGL: software cores never start SDL
//! video nor create a GL context. Keep it alive until the session has dropped
//! (`EmulationApp` declares it after `session`; headless / print-config hold it in
//! `run`).

use hw_render_gl::SdlGlProvider;
use libretro_host::{HwContextKind, HwRenderContext, HwRenderProvider, HwRenderRequest};

/// Debug-build test hook: `NOSTYRA_FORCE_NO_GL=1` makes context creation fail, so
/// process tests can check the "no OpenGL" exit (6) on a PC that has OpenGL;
/// `=refuse` refuses every request instead (`supports` false: the core falls back
/// on its own, e.g. Dolphin's black Null renderer → fatal for `hw_render_required`).
/// Release builds ignore it (no test hook in shipped behaviour).
#[cfg(debug_assertions)]
const FORCE_NO_GL_ENV: &str = "NOSTYRA_FORCE_NO_GL";

/// The session's graphics provider (OpenGL today).
#[derive(Debug, Default)]
pub struct HwRenderSetup {
    gl: SdlGlProvider,
}

impl HwRenderSetup {
    pub fn new() -> Self {
        Self::default()
    }

    /// The provider for `SessionConfig::hw_render`.
    pub fn as_provider(&self) -> &dyn HwRenderProvider {
        self
    }

    /// True once a core made us initialise SDL video (an OpenGL core loaded).
    #[cfg(test)]
    pub fn video_initialised(&self) -> bool {
        self.gl.video_initialised()
    }
}

/// `NOSTYRA_FORCE_NO_GL=<value>` in a debug build.
fn forced(value: &str) -> bool {
    #[cfg(debug_assertions)]
    {
        std::env::var(FORCE_NO_GL_ENV).is_ok_and(|v| v == value)
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = value;
        false
    }
}

impl HwRenderProvider for HwRenderSetup {
    fn supports(&self, request: &HwRenderRequest) -> bool {
        !forced("refuse") && self.gl.supports(request)
    }

    fn preferred(&self) -> HwContextKind {
        self.gl.preferred()
    }

    fn create(
        &self,
        request: &HwRenderRequest,
        max_width: u32,
        max_height: u32,
    ) -> Result<Box<dyn HwRenderContext>, String> {
        if forced("1") {
            return Err("forced by NOSTYRA_FORCE_NO_GL".to_owned());
        }
        self.gl.create(request, max_width, max_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_like_the_gl_backend_without_initialising_sdl() {
        let setup = HwRenderSetup::new();
        let request = HwRenderRequest {
            kind: HwContextKind::OpenGl,
            depth: true,
            stencil: false,
            bottom_left_origin: true,
            cache_context: true,
            debug: false,
        };
        assert!(setup.as_provider().supports(&request));
        let vulkan = HwRenderRequest {
            kind: HwContextKind::Vulkan,
            ..request
        };
        assert!(!setup.as_provider().supports(&vulkan));
        assert!(!setup.video_initialised());
    }
}
