//! Gamepad capture for the Controls editor (backend SDL, see
//! `gamepad_capture_thread`): wait for one press, cancel it, and the label style of
//! the last captured pad. Refused while a game runs so the runner keeps sole use of
//! the pads (no SDL / HIDAPI sharing between processes).

use std::time::Duration;

use tauri::{AppHandle, Manager, State};

use crate::app_state::{lock, AppState};
use crate::command_error::CommandResult;
use crate::commands::blocking::run_blocking;
use crate::gamepad_capture_filter::LabelStyle;
use crate::gamepad_capture_state::GamepadCapture;
use crate::gamepad_capture_thread::CapturedInput;

/// Upper bound of one capture (the UI waits 5 s); longer requests are clamped.
pub const MAX_CAPTURE_MS: u64 = 10_000;

/// Waits up to `timeout_ms` (≤ [`MAX_CAPTURE_MS`]) for a button press or a trigger
/// past half travel; `null` on timeout or cancel. `busy` while a game runs or
/// another capture is active.
#[tauri::command]
pub async fn capture_gamepad_input(
    app: AppHandle,
    timeout_ms: u64,
) -> CommandResult<Option<CapturedInput>> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        // Session re-checked under the capture lock (no race with `play_game`).
        app.state::<GamepadCapture>().capture(
            Duration::from_millis(capture_timeout_ms(timeout_ms)),
            || lock(&state.session).is_some(),
        )
    })
    .await
}

/// Ends a running capture early (it then returns `null`); no-op when idle.
#[tauri::command]
pub fn cancel_gamepad_capture(capture: State<'_, GamepadCapture>) {
    capture.cancel();
}

/// `"xbox"` or `"sony"`: labels of the last captured pad (default Xbox).
#[tauri::command]
pub fn gamepad_label_style(capture: State<'_, GamepadCapture>) -> LabelStyle {
    capture.label_style()
}

fn capture_timeout_ms(requested: u64) -> u64 {
    requested.min(MAX_CAPTURE_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_capped() {
        assert_eq!(capture_timeout_ms(5_000), 5_000);
        assert_eq!(capture_timeout_ms(60_000), MAX_CAPTURE_MS);
    }
}
