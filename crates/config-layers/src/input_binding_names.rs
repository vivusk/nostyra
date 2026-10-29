//! The single owner of binding names: SNES actions, runner hotkeys, physical
//! keyboard / gamepad input names, pad labels and the fixed (reserved) inputs.
//! Runner (`input-mapping` converts names to winit / SDL values) and launcher
//! (validation, Controls UI) both read these tables, so neither hand-keeps a list.
//!
//! Inputs are named by **position**, never by printed label: keyboard names are
//! winit `KeyCode` variant names (physical scancodes, so `KeyZ` is the key left of
//! `X` on any layout), pad names are our own positional snake_case names. SDL
//! mapping strings are deliberately not used: in SDL3 `"a"` means the south
//! button, which would read as "A" to an Xbox user but is "B" on a SNES pad.

/// RetroPad actions (libretro ids, mapped in `input-mapping`), in UI order: the SNES
/// pad, then the second shoulder pair (`l2` / `r2`, PlayStation), the stick clicks
/// (`l3` / `r3`, Nintendo DS extras), then the analog stick directions
/// (`lstick_*` / `rstick_*`: full deflection from a key or button, for systems with
/// `analog_sticks`; on a pad the sticks feed the axes directly).
/// Each system shows its own subset (`SystemSpec.pad_actions`).
#[rustfmt::skip]
pub const PAD_ACTIONS: [&str; 24] = [
    "b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r", "l2", "r2",
    "l3", "r3",
    "lstick_up", "lstick_down", "lstick_left", "lstick_right",
    "rstick_up", "rstick_down", "rstick_left", "rstick_right",
];

/// Runner hotkeys. The order is the conflict priority (an input bound to two
/// hotkeys stays on the first) and the order docs and the UI list them in.
pub const HOTKEYS: [&str; 9] = [
    "pause_menu",
    "save_state",
    "load_state",
    "prev_slot",
    "next_slot",
    "toggle_fullscreen",
    "fast_forward_hold",
    "rewind_hold",
    "turbo_a_toggle",
];

/// Section of keyboard game-button binds (keyboard drives port 1 only).
pub const INPUT_KEYBOARD_SECTION: &str = "input_keyboard";
/// Section of gamepad game-button binds (every pad, ports 1 and 2).
pub const INPUT_GAMEPAD_SECTION: &str = "input_gamepad";
/// Section of keyboard hotkey binds.
pub const HOTKEYS_KEYBOARD_SECTION: &str = "hotkeys_keyboard";
/// Section of gamepad hotkey binds.
pub const HOTKEYS_GAMEPAD_SECTION: &str = "hotkeys_gamepad";

/// Dotted-key prefixes of every input / hotkey key (launcher "Reset controls").
pub const INPUT_KEY_PREFIXES: [&str; 4] = [
    "input_keyboard.",
    "input_gamepad.",
    "hotkeys_keyboard.",
    "hotkeys_gamepad.",
];

/// At most this many inputs per action or hotkey (UI shows two chips).
pub const MAX_BINDS_PER_ACTION: usize = 2;

/// Supported winit `KeyCode` variant names (they equal DOM `KeyboardEvent.code`
/// except `Meta*` → `Super*`). Includes reserved keys so the UI can name them.
#[rustfmt::skip]
pub const KEYBOARD_KEY_NAMES: &[&str] = &[
    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM",
    "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ",
    "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9",
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
    "Insert", "Delete", "Home", "End", "PageUp", "PageDown",
    "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8",
    "Numpad9", "NumpadAdd", "NumpadSubtract", "NumpadMultiply", "NumpadDivide", "NumpadDecimal",
    "NumpadEnter", "NumLock",
    "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "SuperLeft",
    "SuperRight",
    "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "IntlBackslash",
    "Semicolon", "Quote", "Comma", "Period", "Slash",
    "Space", "Enter", "Tab", "Backspace", "Escape", "CapsLock", "ContextMenu", "PrintScreen",
    "ScrollLock", "Pause",
];

/// Positional gamepad input names (SDL gamepad buttons + the two analog triggers).
#[rustfmt::skip]
pub const GAMEPAD_INPUT_NAMES: [&str; 23] = [
    "south", "east", "west", "north", "back", "guide", "start", "left_stick", "right_stick",
    "left_shoulder", "right_shoulder", "dpad_up", "dpad_down", "dpad_left", "dpad_right", "misc1",
    "left_paddle1", "left_paddle2", "right_paddle1", "right_paddle2", "touchpad", "left_trigger",
    "right_trigger",
];

/// What each pad input is printed as: `(name, (xbox, sony))`. Storage stays
/// positional; the UI shows the label of the user's pad style.
#[rustfmt::skip]
pub const GAMEPAD_INPUT_LABELS: [(&str, (&str, &str)); 23] = [
    ("south", ("A", "Cross")), ("east", ("B", "Circle")), ("west", ("X", "Square")),
    ("north", ("Y", "Triangle")), ("back", ("View", "Create")), ("guide", ("Xbox", "PS")),
    ("start", ("Menu", "Options")), ("left_stick", ("LS", "L3")), ("right_stick", ("RS", "R3")),
    ("left_shoulder", ("LB", "L1")), ("right_shoulder", ("RB", "R1")),
    ("dpad_up", ("D-pad Up", "D-pad Up")), ("dpad_down", ("D-pad Down", "D-pad Down")),
    ("dpad_left", ("D-pad Left", "D-pad Left")), ("dpad_right", ("D-pad Right", "D-pad Right")),
    ("misc1", ("Share", "Mic")), ("left_paddle1", ("P3", "Left Paddle 1")),
    ("left_paddle2", ("P4", "Left Paddle 2")), ("right_paddle1", ("P1", "Right Paddle 1")),
    ("right_paddle2", ("P2", "Right Paddle 2")), ("touchpad", ("Touchpad", "Touchpad")),
    ("left_trigger", ("LT", "L2")), ("right_trigger", ("RT", "R2")),
];

/// Keys that always open the pause menu / cancel a capture: never bindable, so a
/// user can never lock themselves out of the menu.
pub const RESERVED_KEYBOARD_KEYS: [&str; 1] = ["Escape"];
/// Pad inputs that always open the pause menu (Guide): never bindable.
pub const RESERVED_GAMEPAD_INPUTS: [&str; 1] = ["guide"];
/// The fixed Select+Start (hold) menu chord uses these physical buttons, so they
/// cannot be pad hotkeys (they stay bindable as game actions).
pub const RESERVED_PAD_HOTKEY_INPUTS: [&str; 2] = ["back", "start"];

/// `(xbox, sony)` label of a pad input name.
pub fn gamepad_input_label(name: &str) -> Option<(&'static str, &'static str)> {
    GAMEPAD_INPUT_LABELS
        .iter()
        .find(|(input, _)| *input == name)
        .map(|(_, labels)| *labels)
}
