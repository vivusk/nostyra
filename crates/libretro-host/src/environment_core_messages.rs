//! Core messages (`SET_MESSAGE` / `SET_MESSAGE_EXT`, e.g. "No PlayStation BIOS file
//! found"): logged, and queued for the frontend's on-screen notice unless the core
//! asked for log only. Drained with `CoreSession::take_core_messages`.
//! Also the core's log interface (`GET_LOG_INTERFACE`) → `tracing`; the last
//! error-level line is kept as the reason of a `SHUTDOWN` without a message (PPSSPP)
//! and of a rejected `retro_load_game` (melonDS DS config errors).

use std::ffi::{c_char, c_uint, c_void, CStr};
use std::sync::Mutex;

use crate::ffi_disc_message_types::*;
use crate::ffi_types::{RetroLogCallback, RETRO_LOG_DEBUG, RETRO_LOG_INFO, RETRO_LOG_WARN};
use crate::host_context::HostContext;

unsafe extern "C" {
    /// C shim (csrc/log-printf-shim.c): formats printf args, forwards to the sink.
    fn nostyra_log_printf(level: c_uint, fmt: *const c_char, ...);
    fn nostyra_set_log_sink(sink: extern "C" fn(level: c_uint, message: *const c_char));
}

/// `GET_LOG_INTERFACE`: hands out the printf shim, which forwards to `tracing`.
///
/// # Safety
/// `data` must be null or a `struct retro_log_callback *`.
pub(crate) unsafe fn log_interface(data: *mut c_void) -> bool {
    // SAFETY: plain store of a fn pointer into the shim's static; emulation thread only.
    unsafe { nostyra_set_log_sink(core_log_sink) };
    let out = data.cast::<RetroLogCallback>();
    if out.is_null() {
        return false;
    }
    // SAFETY: `struct retro_log_callback *` out-param.
    unsafe { (*out).log = nostyra_log_printf };
    true
}

/// Last error-level core log line. Process-wide: cores log from their own threads
/// (PPSSPP's loader), where the thread-local session context is not reachable.
static LAST_CORE_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Error-level lines that are notices, never the reason: melonDS DS logs "Skipping
/// error screen due to the environment variable MELONDSDS_SKIP_ERROR_SCREEN" right
/// after the real error (melonds-ds `src/libretro/core/core.cpp` L423–426 @f394adba).
const NOT_A_REASON: &[&str] = &["Skipping error screen"];

/// Forgets the previous session's last error line (at load).
pub(crate) fn clear_last_core_error() {
    *LAST_CORE_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The last error-level line the core logged, if any.
pub(crate) fn last_core_error() -> Option<String> {
    LAST_CORE_ERROR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Feeds `text` through the sink at error level, as the C shim would.
#[cfg(test)]
pub(crate) fn log_line_for_tests(text: &str) {
    let line = std::ffi::CString::new(text).unwrap_or_default();
    core_log_sink(RETRO_LOG_WARN + 1, line.as_ptr()); // RETRO_LOG_ERROR
}

/// Receives formatted core log lines from the C shim (any thread).
extern "C" fn core_log_sink(level: c_uint, message: *const c_char) {
    let _ = std::panic::catch_unwind(|| {
        if message.is_null() {
            return;
        }
        // SAFETY: shim passes a NUL-terminated stack buffer valid for this call.
        let text = unsafe { CStr::from_ptr(message) }.to_string_lossy();
        let text = text.trim_end();
        match level {
            RETRO_LOG_DEBUG => tracing::debug!(target: "core", "{text}"),
            RETRO_LOG_INFO => tracing::info!(target: "core", "{text}"),
            RETRO_LOG_WARN => tracing::warn!(target: "core", "{text}"),
            _ => {
                tracing::error!(target: "core", "{text}");
                if !text.is_empty() && !NOT_A_REASON.iter().any(|n| text.starts_with(n)) {
                    *LAST_CORE_ERROR.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(text.to_owned());
                }
            }
        }
    });
}

/// Queue bound: a core spamming messages cannot grow memory.
const MAX_QUEUED_MESSAGES: usize = 16;

/// # Safety
/// `data` must be null or point to the type libretro.h documents for `cmd`.
pub(crate) unsafe fn core_message(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    let (msg, on_screen) = match cmd {
        RETRO_ENVIRONMENT_GET_MESSAGE_INTERFACE_VERSION => {
            // SAFETY: `unsigned *` out-param.
            unsafe { data.cast::<c_uint>().write(1) };
            return true;
        }
        // SAFETY: `const struct retro_message *`.
        RETRO_ENVIRONMENT_SET_MESSAGE => (unsafe { (*data.cast::<RetroMessage>()).msg }, true),
        RETRO_ENVIRONMENT_SET_MESSAGE_EXT => {
            // SAFETY: `const struct retro_message_ext *`.
            let ext = unsafe { &*data.cast::<RetroMessageExt>() };
            (ext.msg, ext.target != RETRO_MESSAGE_TARGET_LOG)
        }
        _ => return false,
    };
    // SAFETY: the core passes a NUL-terminated message valid for this call.
    let Some(text) = (unsafe { message_text(msg) }) else {
        return false;
    };
    tracing::info!(target: "core", "message: {text}");
    if on_screen && ctx.core_messages.len() < MAX_QUEUED_MESSAGES {
        ctx.core_messages.push(text);
    }
    true
}

/// # Safety
/// `msg` must be null or NUL-terminated.
unsafe fn message_text(msg: *const c_char) -> Option<String> {
    if msg.is_null() {
        return None;
    }
    // SAFETY: see function contract.
    let text = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
