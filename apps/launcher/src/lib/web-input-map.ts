// How the web edition's player (EmulatorJS 4.2.3 in an iframe) uses the shared
// bindings: Nostyra's positional pad names ↔ the Gamepad API "standard" layout, the
// hotkeys the browser player has, which bound inputs it cannot use (shown as a
// note in the Controls editor, kept in the TOML for the desktop), and the
// `EJS_defaultControls` + key tables handed to the player. Import-free (node --test).

/** Gamepad API standard-layout button index → positional pad input name
 *  (config-layers `GAMEPAD_INPUT_NAMES`). misc1, paddles and touchpad have no
 *  standard button: a browser cannot tell them apart. */
export const STANDARD_BUTTON_INPUTS: readonly string[] = [
  "south", "east", "west", "north", "left_shoulder", "right_shoulder", "left_trigger", "right_trigger",
  "back", "start", "left_stick", "right_stick", "dpad_up", "dpad_down", "dpad_left", "dpad_right", "guide",
];

/** Standard button index of a pad input, or null when the browser has none. */
export function standardButtonIndex(input: string): number | null {
  const index = STANDARD_BUTTON_INPUTS.indexOf(input);
  return index < 0 ? null : index;
}

/** Printed-label style of a pad from its Gamepad API id (Sony vendor 054c or a
 *  PlayStation name → "sony"; everything else Xbox, like the desktop's SDL rule). */
export function labelStyleOfPadId(id: string): "xbox" | "sony" {
  return /vendor:\s*054c|playstation|dualshock|dualsense/i.test(id) ? "sony" : "xbox";
}

/** Hotkeys the browser player has (the rest stay desktop-only and are not listed). */
export const WEB_HOTKEYS: readonly string[] = ["pause_menu", "save_state", "load_state", "fast_forward_hold"];

/** RetroPad action → EmulatorJS control id (libretro joypad ids; analog sticks 16–23). */
const EJS_ACTION_IDS: Record<string, number> = {
  b: 0, y: 1, select: 2, start: 3, up: 4, down: 5, left: 6, right: 7, a: 8, x: 9, l: 10, r: 11,
  l2: 12, r2: 13, l3: 14, r3: 15,
  lstick_right: 16, lstick_left: 17, lstick_down: 18, lstick_up: 19,
  rstick_right: 20, rstick_left: 21, rstick_down: 22, rstick_up: 23,
};

/** EmulatorJS's fast-forward control (held = on). */
export const EJS_FAST_FORWARD_ID = 27;

/** Stick actions: on a pad the sticks drive the analog axes (desktop rule). */
const STICK_ACTIONS = new Set([
  "lstick_right", "lstick_left", "lstick_down", "lstick_up",
  "rstick_right", "rstick_left", "rstick_down", "rstick_up",
]);

/** Players fed by pads: the first four standard-layout pads, in browser index order. */
export const EJS_PLAYERS = 4;

/** Pad inputs of one binding row (`"input_gamepad.a"`) the browser player does not use:
 *  inputs without a standard button (misc1, paddles, touchpad); stick actions follow the
 *  stick itself, so their button binds are not used either. */
export function webUnusedBinds(key: string, binds: readonly string[]): string[] {
  const [section, name = ""] = key.split(".");
  if (section === "input_gamepad") {
    if (STICK_ACTIONS.has(name)) return [...binds];
    return binds.filter((input) => standardButtonIndex(input) === null);
  }
  if (section === "hotkeys_gamepad") return binds.filter((input) => standardButtonIndex(input) === null);
  return [];
}

/** What the player iframe gets. Nostyra reads keyboard and pads itself (by physical
 *  `KeyboardEvent.code` and the Gamepad API standard layout, like the desktop: every
 *  bound input works, several per button); EmulatorJS's own pad handling is off (its
 *  4.2.3 build confuses pad indices with list positions). */
export interface PlayerInput {
  /** Port 1 keyboard: `KeyboardEvent.code` → EmulatorJS control ids. */
  keys: Record<string, number[]>;
  /** Every pad: standard button index → EmulatorJS control ids. */
  padButtons: Record<number, number[]>;
  /** Analog sticks the system reads (`left`/`right`); without a left stick it acts as
   *  the D-pad (desktop rule for systems without analog input). */
  sticks: { left: boolean; right: boolean };
  /** Web hotkeys → keyboard codes and standard pad button indices. */
  hotkeys: { keyboard: Record<string, string[]>; gamepad: Record<string, number[]> };
}

/** Builds the player's input from resolved values (`"input_keyboard.b": ["KeyZ"]`)
 *  for a system's pad actions; `codeOfKeyName` maps a stored key name to its DOM
 *  code (keyboard-event-code.ts, passed in to keep this module import-free). */
export function playerInput(
  values: Record<string, unknown>,
  padActions: readonly string[],
  codeOfKeyName: (name: string) => string,
): PlayerInput {
  const binds = (key: string): string[] => (Array.isArray(values[key]) ? (values[key] as string[]) : []);
  const keys: Record<string, number[]> = {};
  const padButtons: Record<number, number[]> = {};
  for (const action of padActions) {
    const id = EJS_ACTION_IDS[action];
    if (id === undefined) continue;
    for (const name of binds(`input_keyboard.${action}`)) (keys[codeOfKeyName(name)] ??= []).push(id);
    if (STICK_ACTIONS.has(action)) continue;
    for (const input of binds(`input_gamepad.${action}`)) {
      const index = standardButtonIndex(input);
      if (index !== null) (padButtons[index] ??= []).push(id);
    }
  }
  const sticks = {
    left: padActions.some((action) => action.startsWith("lstick_")),
    right: padActions.some((action) => action.startsWith("rstick_")),
  };
  const hotkeys: PlayerInput["hotkeys"] = { keyboard: {}, gamepad: {} };
  for (const hotkey of WEB_HOTKEYS) {
    hotkeys.keyboard[hotkey] = binds(`hotkeys_keyboard.${hotkey}`).map(codeOfKeyName);
    hotkeys.gamepad[hotkey] = binds(`hotkeys_gamepad.${hotkey}`)
      .map(standardButtonIndex)
      .filter((index): index is number => index !== null);
  }
  return { keys, padButtons, sticks, hotkeys };
}
