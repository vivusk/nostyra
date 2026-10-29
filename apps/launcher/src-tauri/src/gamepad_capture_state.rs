//! Managed capture state: at most one capture at a time, never while a game is
//! starting or running (the runner must own the pads alone: no SDL / HIDAPI
//! sharing between processes), and the label style of the last captured pad.
//!
//! Play/capture race: `play_game` takes a [`PlayBlock`] *before* claiming the
//! session. Taking it cancels a running capture and waits until its thread has
//! dropped SDL; while it is held, and afterwards while the session runs (checked
//! under the same lock), no capture can start.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::app_state::lock;
use crate::command_error::{CommandError, CommandResult};
use crate::gamepad_capture_filter::LabelStyle;
use crate::gamepad_capture_thread::{run_capture, CapturedInput};

/// Longest `play_game` waits for a cancelled capture to release SDL (normally
/// one 50 ms poll slice plus SDL quit).
const CAPTURE_STOP_WAIT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Slots {
    /// Cancel flag of the running capture; `None` = idle.
    running: Option<Arc<AtomicBool>>,
    /// Number of `play_game` calls currently starting a game.
    play_blocks: usize,
}

#[derive(Default)]
pub struct GamepadCapture {
    slots: Mutex<Slots>,
    /// Signalled when a capture ends.
    idle: Condvar,
    label_style: Mutex<LabelStyle>,
}

/// Held by `play_game` while it starts a game; captures are refused meanwhile.
pub struct PlayBlock<'a>(&'a GamepadCapture);

impl Drop for PlayBlock<'_> {
    fn drop(&mut self) {
        let mut slots = lock(&self.0.slots);
        slots.play_blocks = slots.play_blocks.saturating_sub(1);
    }
}

impl GamepadCapture {
    /// Blocks until a press (`Some`), `timeout` or cancel (`None`). `busy` while
    /// another capture runs, a game is starting, or `session_running()` (checked
    /// under the capture lock, so it cannot race `play_game`); `gamepad` when SDL
    /// cannot start.
    pub fn capture(
        &self,
        timeout: Duration,
        session_running: impl Fn() -> bool,
    ) -> CommandResult<Option<CapturedInput>> {
        let cancel = {
            let mut slots = lock(&self.slots);
            if slots.play_blocks > 0 || session_running() {
                return Err(CommandError::new(
                    "busy",
                    "Close the game first: it is using the gamepads.",
                ));
            }
            if slots.running.is_some() {
                return Err(CommandError::new(
                    "busy",
                    "A gamepad capture is already running.",
                ));
            }
            let cancel = Arc::new(AtomicBool::new(false));
            slots.running = Some(cancel.clone());
            cancel
        };
        let result = std::thread::Builder::new()
            .name("gamepad-capture".into())
            .spawn(move || run_capture(timeout, &cancel))
            .map_err(|err| err.to_string())
            .and_then(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("capture thread panicked".into()))
            });
        // The thread has returned: SDL is dropped before anyone sees "idle".
        lock(&self.slots).running = None;
        self.idle.notify_all();
        let captured = result.map_err(|err| {
            tracing::warn!(%err, "gamepad capture failed");
            CommandError::new("gamepad", format!("Cannot read gamepads: {err}"))
        })?;
        if let Some(captured) = &captured {
            *lock(&self.label_style) = captured.label_style;
        }
        Ok(captured)
    }

    /// Ends the running capture (if any) with `None` within one poll slice.
    pub fn cancel(&self) {
        if let Some(cancel) = lock(&self.slots).running.as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// For `play_game`: blocks new captures, cancels a running one and waits until
    /// it has fully finished. `busy` if it did not stop within [`CAPTURE_STOP_WAIT`].
    pub fn block_for_play(&self) -> CommandResult<PlayBlock<'_>> {
        let mut slots = lock(&self.slots);
        slots.play_blocks += 1;
        let block = PlayBlock(self);
        if let Some(cancel) = slots.running.as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
        let (slots, _) = self
            .idle
            .wait_timeout_while(slots, CAPTURE_STOP_WAIT, |s| s.running.is_some())
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slots.running.is_some() {
            drop(slots);
            return Err(CommandError::new(
                "busy",
                "The gamepad capture is still closing; try again.",
            ));
        }
        drop(slots);
        Ok(block)
    }

    /// Label style of the last captured pad (Xbox until a capture says otherwise).
    pub fn label_style(&self) -> LabelStyle {
        *lock(&self.label_style)
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        lock(&self.slots).running.is_some()
    }
}

#[cfg(test)]
#[path = "gamepad_capture_state_tests.rs"]
mod tests;
