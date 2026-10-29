//! [`probe_gl_context`]: can this machine create the OpenGL 3.3 context a core needs?
//! Used by test gates (skip without GL) and diagnostics.

use libretro_host::{HwContextKind, HwRenderProvider, HwRenderRequest};

use crate::sdl_gl_provider::SdlGlProvider;

/// Creates and drops a 3.3 core context with a small framebuffer; the error text says
/// why it failed (no driver, SDL thread rule, ...).
pub fn probe_gl_context() -> Result<(), String> {
    let request = HwRenderRequest {
        kind: HwContextKind::OpenGlCore { major: 3, minor: 3 },
        depth: true,
        stencil: false,
        bottom_left_origin: true,
        cache_context: true,
        debug: false,
    };
    SdlGlProvider::new().create(&request, 64, 64).map(drop)
}
