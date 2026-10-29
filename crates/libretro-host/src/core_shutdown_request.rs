//! `RETRO_ENVIRONMENT_SHUTDOWN`: the core gives up (Flycast after a fatal emulator
//! error, right after a `SET_MESSAGE` saying why). The request is accepted and the
//! next `run_frame` returns [`HostError::CoreShutdown`] with that message (PPSSPP
//! sends none: its last error-level log line instead), so the frontend stops
//! instead of showing a frozen image.

use crate::environment_core_messages::last_core_error;
use crate::host_context::HostContext;
use crate::host_error::{HostError, HostResult};

pub(crate) fn request_shutdown(ctx: &mut HostContext) -> bool {
    tracing::warn!("core requested shutdown");
    ctx.shutdown_requested = true;
    true
}

impl HostContext {
    /// After `retro_run`: a panicked callback or a shutdown request ends the frame
    /// with an error (a shutdown stays an error on every later frame).
    pub(crate) fn take_frame_error(&mut self) -> HostResult<()> {
        if std::mem::take(&mut self.callback_panicked) {
            return Err(HostError::CallbackPanicked);
        }
        if self.shutdown_requested {
            let reason = self.core_messages.last().cloned().or_else(last_core_error);
            return Err(HostError::CoreShutdown(reason));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_becomes_a_frame_error_carrying_the_last_message() {
        let mut ctx = HostContext::new(c"".into(), c"".into());
        assert!(ctx.take_frame_error().is_ok());
        ctx.core_messages
            .push("Fatal: SH4 exception when blocked".into());
        assert!(request_shutdown(&mut ctx));
        let err = ctx.take_frame_error().unwrap_err();
        assert_eq!(
            err.to_string(),
            "the core stopped after an emulation error: Fatal: SH4 exception when blocked"
        );
        assert!(ctx.take_frame_error().is_err(), "stays stopped");
        let bare = HostError::CoreShutdown(None).to_string();
        assert_eq!(bare, "the core stopped after an emulation error");
    }

    #[test]
    fn shutdown_without_a_message_carries_the_last_error_log_line() {
        let _serial = crate::hw_render_test_fakes::serial(); // process-wide last error line
        let mut ctx = HostContext::new(c"".into(), c"".into());
        crate::environment_core_messages::log_line_for_tests("Failed to load executable: bad ELF");
        assert!(request_shutdown(&mut ctx));
        let err = ctx.take_frame_error().unwrap_err().to_string();
        assert!(
            err.ends_with(": Failed to load executable: bad ELF"),
            "{err}"
        );
    }
}
