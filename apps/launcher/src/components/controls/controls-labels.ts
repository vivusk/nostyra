// Display labels for the Controls editor. Names, order and defaults come from the
// backend catalog (`get_input_catalog`); this file only turns names into words.
import type { BindingKey } from "../../api/tauri-commands-gameplay";
import type { InputCatalog, InputDevice, LabelStyle } from "../../api/tauri-commands-input";
import type { SystemView } from "../../api/tauri-commands-systems";

/** RetroPad button labels by catalog action name (SNES names; other pads show the
 *  subset their system lists, e.g. NES: B, A, Select, Start, D-pad). L3 / R3 are the
 *  stick clicks (Nintendo DS). Stick actions are keys / buttons that push an analog
 *  stick fully (systems with analog sticks). */
const PAD_ACTION_LABELS: Record<string, string> = {
  b: "B",
  y: "Y",
  select: "Select",
  start: "Start",
  up: "Up",
  down: "Down",
  left: "Left",
  right: "Right",
  a: "A",
  x: "X",
  l: "L",
  r: "R",
  l2: "L2",
  r2: "R2",
  l3: "L3",
  r3: "R3",
  lstick_up: "Left stick ↑",
  lstick_down: "Left stick ↓",
  lstick_left: "Left stick ←",
  lstick_right: "Left stick →",
  rstick_up: "Right stick ↑",
  rstick_down: "Right stick ↓",
  rstick_left: "Right stick ←",
  rstick_right: "Right stick →",
};

/** Human labels of the runner hotkeys. */
const HOTKEY_LABELS: Record<string, string> = {
  save_state: "Save state",
  load_state: "Load state",
  prev_slot: "Previous slot",
  next_slot: "Next slot",
  toggle_fullscreen: "Toggle fullscreen",
  pause_menu: "Pause menu",
  fast_forward_hold: "Fast-forward (hold)",
  rewind_hold: "Rewind (hold)",
  turbo_a_toggle: "Turbo A (on/off)",
};

/** Punctuation keys (winit / DOM code → printed character). */
const KEY_SYMBOLS: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  SuperLeft: "Left Win",
  SuperRight: "Right Win",
  ControlLeft: "Left Ctrl",
  ControlRight: "Right Ctrl",
};

/** The config sections a device edits: game buttons and shortcuts. */
export function deviceSections(device: InputDevice) {
  return device === "keyboard"
    ? { actions: "input_keyboard", hotkeys: "hotkeys_keyboard" }
    : { actions: "input_gamepad", hotkeys: "hotkeys_gamepad" };
}

/** "input_keyboard.b" → "B"; "hotkeys_gamepad.save_state" → "Save state". */
export function bindingKeyLabel(key: BindingKey): string {
  const [section, name = ""] = key.split(".");
  const labels = section.startsWith("hotkeys_") ? HOTKEY_LABELS : PAD_ACTION_LABELS;
  return labels[name] ?? name;
}

/** Game-button label as the shown system names it, with the shared RetroPad name in
 *  brackets ("A (Y)" on the Mega Drive); the RetroPad label when the system has no
 *  own name for it or the same one (GameCube A stays "A", not "A (A)"). Hotkeys keep
 *  their label. */
export function systemActionLabel(key: BindingKey, system?: SystemView): string {
  const label = bindingKeyLabel(key);
  const [section, name] = key.split(".");
  if (section.startsWith("hotkeys_")) return label;
  const own = system?.padLabels.find((entry) => entry.action === name)?.label;
  return own && own !== label ? `${own} (${label})` : label;
}

/** winit key name → printed label: KeyZ → "Z", Digit1 → "1", ShiftRight → "Right Shift",
 *  ArrowUp → "Arrow Up", Numpad4 → "Num 4", PageDown → "Page Down". */
export function keyLabel(name: string): string {
  const symbol = KEY_SYMBOLS[name];
  if (symbol) return symbol;
  const letterOrDigit = /^(?:Key|Digit)(.)$/.exec(name);
  if (letterOrDigit) return letterOrDigit[1];
  const sided = /^(.+)(Left|Right)$/.exec(name);
  if (sided && !name.startsWith("Arrow")) return `${sided[2]} ${spaced(sided[1])}`;
  return spaced(name.replace(/^Numpad/, "Num "));
}

/** "PageDown" → "Page Down" (split before capitals and before digits). */
const spaced = (text: string) => text.replace(/([a-z])([A-Z0-9])/g, "$1 $2");

/** Printed label of a pad input in the current style (Xbox / Sony); name when unknown. */
export function gamepadLabel(name: string, catalog: InputCatalog, style: LabelStyle): string {
  const info = catalog.gamepadInputs.find((input) => input.name === name);
  return info ? info[style] : name;
}

/** Chip label of one bound input. */
export function inputLabel(device: InputDevice, name: string, catalog: InputCatalog, style: LabelStyle): string {
  return device === "keyboard" ? keyLabel(name) : gamepadLabel(name, catalog, style);
}

/** Why a captured input cannot go on `target` (shown in the capture dialog), or null. */
export function rejectReason(device: InputDevice, target: BindingKey, input: string, catalog: InputCatalog): string | null {
  if (device === "keyboard") {
    if (catalog.reservedKeys.includes(input)) return "Esc always opens the pause menu and cannot be bound.";
    return null;
  }
  if (catalog.reservedGamepadInputs.includes(input)) return "Guide always opens the pause menu and cannot be bound.";
  if (target.startsWith("hotkeys_") && catalog.reservedPadHotkeyInputs.includes(input)) {
    return "Select and Start are kept for the pause-menu chord (hold both). Bind them to game buttons only.";
  }
  return null;
}
