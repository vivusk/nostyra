//! `retro_environment` handler: the core's channel for querying/setting frontend
//! features. Unknown commands return `false` (core falls back to defaults).

use std::ffi::{c_char, c_int, c_uint, c_void};

use crate::core_shutdown_request::request_shutdown;
use crate::environment_av_info::av_info_change;
use crate::environment_core_messages::{core_message, log_interface};
use crate::environment_core_options::get_variable;
use crate::environment_disk_control::disk_control;
use crate::environment_hw_render::hw_render;
use crate::environment_memory_maps::{
    memory_maps, RETRO_ENVIRONMENT_SET_MEMORY_MAPS as SET_MEMORY_MAPS,
    RETRO_ENVIRONMENT_SET_SUPPORT_ACHIEVEMENTS as SET_SUPPORT_ACHIEVEMENTS,
};
use crate::ffi_disc_message_types::*;
use crate::ffi_hw_render_types::{
    RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER, RETRO_ENVIRONMENT_SET_HW_RENDER,
};
use crate::ffi_types::*;
use crate::frame_sink_traits::PixelFormat;
use crate::host_context::{with_current, HostContext};
use crate::savestate_context::SavestateContext;
use crate::serialization_quirks::SerializationQuirks;

/// Registered with the core via `retro_set_environment`.
pub(crate) unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    let cmd = cmd & !RETRO_ENVIRONMENT_EXPERIMENTAL;
    with_current(false, |ctx| {
        // SAFETY: libretro.h defines `data`'s pointee type per command; each arm
        // casts to exactly that type and null-checks before use.
        unsafe { handle(ctx, cmd, data) }
    })
}

/// # Safety
/// `data` must point to the type libretro.h documents for `cmd` (or be null).
unsafe fn handle(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    match cmd {
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY | RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY => {
            let dir = if cmd == RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY {
                &ctx.system_dir
            } else {
                &ctx.save_dir
            };
            let out = data.cast::<*const c_char>();
            if out.is_null() {
                return false;
            }
            // SAFETY: `const char **` out-param; the CString outlives the session.
            unsafe { *out = dir.as_ptr() };
            true
        }
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            let fmt = data.cast::<c_uint>();
            if fmt.is_null() {
                return false;
            }
            // SAFETY: `const enum retro_pixel_format *` (32-bit enum).
            let requested = match unsafe { *fmt } {
                RETRO_PIXEL_FORMAT_0RGB1555 => PixelFormat::Rgb1555,
                RETRO_PIXEL_FORMAT_XRGB8888 => PixelFormat::Xrgb8888,
                RETRO_PIXEL_FORMAT_RGB565 => PixelFormat::Rgb565,
                _ => return false,
            };
            ctx.pixel_format = requested;
            true
        }
        RETRO_ENVIRONMENT_GET_CAN_DUPE => write_out(data.cast::<bool>(), true),
        RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE => write_out(data.cast::<c_int>(), av_flags(ctx)),
        RETRO_ENVIRONMENT_GET_SAVESTATE_CONTEXT => {
            // NULL is an availability probe: still "supported".
            let out = data.cast::<c_int>();
            out.is_null() || write_out(out, ctx.savestate_context.as_raw())
        }
        RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS
        | RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS_2 => {
            let bits = data.cast::<u64>();
            if bits.is_null() {
                return false;
            }
            // SAFETY: `uint64_t *` in/out-param.
            let (quirks, reply) = SerializationQuirks::negotiate(unsafe { *bits });
            tracing::debug!(?quirks, "core serialization quirks");
            ctx.serialization_quirks = quirks;
            write_out(bits, reply)
        }
        RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            // 0 = legacy SET_VARIABLES only: no option UI, defaults + fixed options.
            write_out(data.cast::<c_uint>(), 0)
        }
        RETRO_ENVIRONMENT_GET_VARIABLE => {
            // SAFETY: `data` is a `struct retro_variable *` (or null).
            unsafe { get_variable(ctx, data) }
        }
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => false,
        RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION
        | RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE
        | RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE => {
            // SAFETY: `data` is the type libretro.h documents for `cmd` (or null).
            unsafe { disk_control(ctx, cmd, data) }
        }
        RETRO_ENVIRONMENT_GET_MESSAGE_INTERFACE_VERSION
        | RETRO_ENVIRONMENT_SET_MESSAGE
        | RETRO_ENVIRONMENT_SET_MESSAGE_EXT => {
            // SAFETY: as above.
            unsafe { core_message(ctx, cmd, data) }
        }
        RETRO_ENVIRONMENT_SET_VARIABLES
        | RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
        | RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL => true,
        // SAFETY: `struct retro_log_callback *` out-param (or null).
        RETRO_ENVIRONMENT_GET_LOG_INTERFACE => unsafe { log_interface(data) },
        RETRO_ENVIRONMENT_SET_HW_RENDER | RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER => {
            // SAFETY: `data` is the type libretro.h documents for `cmd` (or null).
            unsafe { hw_render(ctx, cmd, data) }
        }
        RETRO_ENVIRONMENT_SHUTDOWN => request_shutdown(ctx),
        RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO | RETRO_ENVIRONMENT_SET_GEOMETRY => {
            // SAFETY: `data` is the type libretro.h documents for `cmd` (or null).
            unsafe { av_info_change(ctx, cmd, data) }
        }
        // SAFETY: `data` is the type libretro.h documents for `cmd` (or null).
        SET_MEMORY_MAPS | SET_SUPPORT_ACHIEVEMENTS => unsafe { memory_maps(ctx, cmd, data) },
        _ => {
            tracing::trace!(cmd, "unsupported environment command");
            false
        }
    }
}

/// `GET_AUDIO_VIDEO_ENABLE` reply: the enabled outputs, plus `FAST_SAVESTATES` while a
/// run-ahead/rewind state is being taken or loaded (older cores check it there).
fn av_flags(ctx: &HostContext) -> c_int {
    let mut flags = 0;
    if ctx.av_enable.video {
        flags |= RETRO_AV_ENABLE_VIDEO;
    }
    if ctx.av_enable.audio {
        flags |= RETRO_AV_ENABLE_AUDIO;
    }
    if ctx.savestate_context != SavestateContext::Normal {
        flags |= RETRO_AV_ENABLE_FAST_SAVESTATES;
    }
    flags
}

/// Writes `value` to a nullable out-param; `false` if null.
fn write_out<T>(out: *mut T, value: T) -> bool {
    if out.is_null() {
        return false;
    }
    // SAFETY: non-null out-param of type T as documented for the command.
    unsafe { out.write(value) };
    true
}

#[cfg(test)]
#[path = "environment_callback_tests.rs"]
mod tests;
