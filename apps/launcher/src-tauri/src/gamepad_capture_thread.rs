//! Gamepad capture for the Controls editor. SDL lives only for one capture, on a
//! dedicated thread that creates and drops the `Sdl` handle itself: `sdl3` marks
//! the first init thread as SDL's "main" thread until the last handle drops, so a
//! handle kept across captures (or created on a pooled thread) would pin it. Only
//! the gamepad subsystem starts (no video), with background joystick events on,
//! because the launcher's window, not SDL, has the focus. Exclusivity and the
//! Play race live in `gamepad_capture_state`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sdl3::event::Event;
use sdl3::gamepad::Gamepad;
use sdl3::GamepadSubsystem;
use serde::Serialize;

use crate::gamepad_capture_filter::{
    CaptureFilter, LabelStyle, BUTTON_NAMES, TRIGGER_NAMES, TRIGGER_PRESS_THRESHOLD,
};

/// Longest wait between cancel / deadline checks.
const POLL_SLICE: Duration = Duration::from_millis(50);

/// What a capture returns: the pressed input and the label style of its pad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedInput {
    pub input: &'static str,
    pub label_style: LabelStyle,
}

/// One capture on the current (dedicated) thread; SDL quits when it returns.
pub(crate) fn run_capture(
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<Option<CapturedInput>, String> {
    let started = Instant::now();
    sdl3::hint::set("SDL_WINDOWS_ENABLE_MESSAGELOOP", "0");
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    let sdl = sdl3::init().map_err(|err| err.to_string())?;
    let subsystem = sdl.gamepad().map_err(|err| err.to_string())?;
    let mut pump = sdl.event_pump().map_err(|err| err.to_string())?;
    tracing::debug!(
        init_ms = started.elapsed().as_millis(),
        "gamepad capture started"
    );
    let deadline = started + timeout;
    let mut pads: HashMap<u32, Gamepad> = HashMap::new();
    let mut filter = CaptureFilter::default();
    while !cancel.load(Ordering::Relaxed) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let Some(event) = pump.wait_event_timeout(left.min(POLL_SLICE)) else {
            continue;
        };
        let pressed = match event {
            Event::GamepadAdded { which, .. } => {
                open_pad(&subsystem, which.into(), &mut pads, &mut filter);
                None
            }
            Event::GamepadRemoved { which, .. } => {
                pads.remove(&which.into());
                None
            }
            Event::GamepadButtonDown { which, button, .. } => filter
                .button_down(which.into(), button)
                .map(|n| (which.into(), n)),
            Event::GamepadButtonUp { which, button, .. } => {
                filter.button_up(which.into(), button);
                None
            }
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => filter
                .axis(which.into(), axis, value)
                .map(|n| (which.into(), n)),
            _ => None,
        };
        if let Some((pad, input)) = pressed {
            let label_style = pads
                .get(&pad)
                .map_or(LabelStyle::Xbox, |p| LabelStyle::of(p.r#type()));
            tracing::info!(input, ?label_style, "gamepad input captured");
            return Ok(Some(CapturedInput { input, label_style }));
        }
    }
    Ok(None)
}

/// Opens a newly seen pad (keeps it alive for its events and type) and ignores
/// whatever is already held on it.
fn open_pad(
    subsystem: &GamepadSubsystem,
    id: u32,
    pads: &mut HashMap<u32, Gamepad>,
    filter: &mut CaptureFilter,
) {
    let pad = match subsystem.open(id.into()) {
        Ok(pad) => pad,
        Err(err) => {
            tracing::warn!(%err, id, "cannot open gamepad for capture");
            return;
        }
    };
    for (button, name) in BUTTON_NAMES {
        if pad.button(button) {
            filter.ignore_held(id, name);
        }
    }
    for (axis, name) in TRIGGER_NAMES {
        if pad.axis(axis) >= TRIGGER_PRESS_THRESHOLD {
            filter.ignore_held(id, name);
        }
    }
    tracing::debug!(id, name = ?pad.name(), kind = ?pad.r#type(), "gamepad opened for capture");
    pads.insert(id, pad);
}
