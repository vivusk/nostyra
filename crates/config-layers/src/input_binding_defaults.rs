//! Built-in binds of the four binding sections (the lowest merge layer). Every
//! action / hotkey is listed, in [`crate::PAD_ACTIONS`] / [`crate::HOTKEYS`] order.

/// One section's built-in binds: `(action or hotkey, inputs)`.
pub type DefaultBinds = [(&'static str, &'static [&'static str])];

/// Keyboard game buttons: arrows, Z=B, X=A, A=Y, S=X, Q/W shoulders, E/R second
/// shoulders, Enter=Start, Right Shift=Select (the M1–M3 layout + L2/R2); L3/R3
/// unbound (the user may bind them); I/K/J/L left stick, T/G/F/H right stick (N64
/// C-buttons).
pub const DEFAULT_INPUT_KEYBOARD: &DefaultBinds = &[
    ("b", &["KeyZ"]),
    ("y", &["KeyA"]),
    ("select", &["ShiftRight"]),
    ("start", &["Enter"]),
    ("up", &["ArrowUp"]),
    ("down", &["ArrowDown"]),
    ("left", &["ArrowLeft"]),
    ("right", &["ArrowRight"]),
    ("a", &["KeyX"]),
    ("x", &["KeyS"]),
    ("l", &["KeyQ"]),
    ("r", &["KeyW"]),
    ("l2", &["KeyE"]),
    ("r2", &["KeyR"]),
    ("l3", &[]),
    ("r3", &[]),
    ("lstick_up", &["KeyI"]),
    ("lstick_down", &["KeyK"]),
    ("lstick_left", &["KeyJ"]),
    ("lstick_right", &["KeyL"]),
    ("rstick_up", &["KeyT"]),
    ("rstick_down", &["KeyG"]),
    ("rstick_left", &["KeyF"]),
    ("rstick_right", &["KeyH"]),
];

/// Pad game buttons by Nintendo position (south = B, east = A, …); L2/R2 on the
/// triggers (shared with the hold hotkeys, see `input_conflicts`); L3/R3 on the
/// stick clicks. Stick actions start unbound: on a pad the sticks themselves feed the analog axes.
pub const DEFAULT_INPUT_GAMEPAD: &DefaultBinds = &[
    ("b", &["south"]),
    ("y", &["west"]),
    ("select", &["back"]),
    ("start", &["start"]),
    ("up", &["dpad_up"]),
    ("down", &["dpad_down"]),
    ("left", &["dpad_left"]),
    ("right", &["dpad_right"]),
    ("a", &["east"]),
    ("x", &["north"]),
    ("l", &["left_shoulder"]),
    ("r", &["right_shoulder"]),
    ("l2", &["left_trigger"]),
    ("r2", &["right_trigger"]),
    ("l3", &["left_stick"]),
    ("r3", &["right_stick"]),
    ("lstick_up", &[]),
    ("lstick_down", &[]),
    ("lstick_left", &[]),
    ("lstick_right", &[]),
    ("rstick_up", &[]),
    ("rstick_down", &[]),
    ("rstick_left", &[]),
    ("rstick_right", &[]),
];

/// F10 menu (Esc is fixed on top), F1 save, F4 load, F2/F3 slots, F11
/// fullscreen, `` ` `` fast-forward hold, Backspace rewind hold, P turbo A on/off.
pub const DEFAULT_HOTKEYS_KEYBOARD: &DefaultBinds = &[
    ("pause_menu", &["F10"]),
    ("save_state", &["F1"]),
    ("load_state", &["F4"]),
    ("prev_slot", &["F2"]),
    ("next_slot", &["F3"]),
    ("toggle_fullscreen", &["F11"]),
    ("fast_forward_hold", &["Backquote"]),
    ("rewind_hold", &["Backspace"]),
    ("turbo_a_toggle", &["KeyP"]),
];

/// Triggers hold fast-forward / rewind; the rest start unbound (Guide and the
/// Select+Start chord open the menu without a binding).
pub const DEFAULT_HOTKEYS_GAMEPAD: &DefaultBinds = &[
    ("pause_menu", &[]),
    ("save_state", &[]),
    ("load_state", &[]),
    ("prev_slot", &[]),
    ("next_slot", &[]),
    ("toggle_fullscreen", &[]),
    ("fast_forward_hold", &["right_trigger"]),
    ("rewind_hold", &["left_trigger"]),
    ("turbo_a_toggle", &[]),
];
