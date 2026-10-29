//! `SET_SYSTEM_AV_INFO` / `SET_GEOMETRY`: the core changed its timing or frame size
//! (Flycast does both mid-`retro_run`, e.g. when the game switches resolution).
//!
//! The runner picks the new av info up after the frame (`take_av_change`). A hardware
//! framebuffer is sized to the av maximum once at load; a `SET_SYSTEM_AV_INFO` whose
//! maximum exceeds it grows it here, inside the core call (the context is current),
//! so the next frame is not cropped. `SET_GEOMETRY` may not raise the maximum
//! (libretro.h), so it never resizes.

use std::ffi::{c_uint, c_void};

use crate::ffi_types::*;
use crate::host_context::HostContext;

/// # Safety
/// `data` must be null or point to the type libretro.h documents for `cmd`.
pub(crate) unsafe fn av_info_change(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    if cmd == RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO {
        // SAFETY: `const struct retro_system_av_info *`.
        ctx.av_info = unsafe { *data.cast::<RetroSystemAvInfo>() };
        let geometry = ctx.av_info.geometry;
        grow_framebuffer(ctx, geometry.max_width, geometry.max_height);
    } else {
        // SAFETY: `const struct retro_game_geometry *`.
        ctx.av_info.geometry = unsafe { *data.cast::<RetroGameGeometry>() };
    }
    ctx.av_changed = true;
    true
}

/// Grows the hardware framebuffer to cover `max_width × max_height` (never shrinks:
/// the larger storage serves smaller frames). A failed resize keeps the old size;
/// frames are then cropped (`deliver_frame`), which is better than stopping.
fn grow_framebuffer(ctx: &mut HostContext, max_width: u32, max_height: u32) {
    let Some(context) = ctx.hw_context.as_mut() else {
        return;
    };
    let (width, height) = context.size();
    if max_width <= width && max_height <= height {
        return;
    }
    let (new_w, new_h) = (max_width.max(width), max_height.max(height));
    match context.resize(new_w, new_h) {
        Ok(()) => tracing::info!(new_w, new_h, "hardware framebuffer grown"),
        Err(error) => tracing::error!(%error, new_w, new_h, "cannot grow the framebuffer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw_render_test_fakes::{log_take, serial, FakeContext};

    fn ctx_with_fake(width: u32, height: u32) -> HostContext {
        let mut ctx = HostContext::new(c"".into(), c"".into());
        ctx.hw_context = Some(Box::new(FakeContext {
            size: (width, height),
        }));
        ctx
    }

    fn send_av(ctx: &mut HostContext, max_w: u32, max_h: u32) {
        let mut av = RetroSystemAvInfo::default();
        av.geometry.max_width = max_w;
        av.geometry.max_height = max_h;
        // SAFETY: valid av info for SET_SYSTEM_AV_INFO.
        let ok = unsafe {
            av_info_change(
                ctx,
                RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO,
                (&raw mut av).cast(),
            )
        };
        assert!(ok);
    }

    #[test]
    fn av_info_resizes_only_when_the_maximum_grows() {
        let _serial = serial();
        let mut ctx = ctx_with_fake(853, 853);
        send_av(&mut ctx, 853, 853);
        send_av(&mut ctx, 640, 480);
        assert!(log_take().is_empty(), "equal or smaller: no resize");
        send_av(&mut ctx, 1706, 1706);
        send_av(&mut ctx, 1706, 1000);
        assert_eq!(log_take(), ["resize 1706x1706"]);
        assert!(ctx.av_changed);
        assert_eq!(ctx.av_info.geometry.max_width, 1706);
        // One axis larger: grows to cover both.
        send_av(&mut ctx, 1000, 1800);
        assert_eq!(log_take(), ["resize 1706x1800"]);
        drop(ctx);
        log_take();
    }

    #[test]
    fn geometry_never_resizes_and_software_sessions_are_untouched() {
        let _serial = serial();
        let mut ctx = ctx_with_fake(640, 480);
        let mut geometry = RetroGameGeometry {
            base_width: 1280,
            base_height: 960,
            max_width: 1280,
            max_height: 960,
            aspect_ratio: 4.0 / 3.0,
        };
        // SAFETY: valid geometry for SET_GEOMETRY.
        assert!(unsafe {
            av_info_change(
                &mut ctx,
                RETRO_ENVIRONMENT_SET_GEOMETRY,
                (&raw mut geometry).cast(),
            )
        });
        assert_eq!(ctx.av_info.geometry.base_width, 1280);
        let mut software = HostContext::new(c"".into(), c"".into());
        send_av(&mut software, 4000, 4000);
        drop(ctx);
        assert_eq!(
            log_take(),
            ["drop context"],
            "no resize without growth path"
        );
    }
}
