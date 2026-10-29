// Typed wrappers for the Controls editor commands
// (src-tauri/src/commands/{input_catalog,gamepad_capture}_commands.rs). Names,
// labels and defaults come from the backend catalog; nothing is hand-listed here.
import { invoke } from "@tauri-apps/api/core";
import type { BindingKey, GameplayConfigScope } from "./tauri-commands-gameplay";

export type InputDevice = "keyboard" | "gamepad";
/** Which printed pad labels to show (last captured pad). */
export type LabelStyle = "xbox" | "sony";

export interface GamepadInputInfo {
  /** Positional name stored in config (`south`, `right_trigger`, …). */
  name: string;
  xbox: string;
  sony: string;
}

export interface InputCatalog {
  /** Every RetroPad button in UI order (`b y select start up down left right a x l r l2 r2`);
   *  each system shows its own subset (`SystemView.padActions`). */
  padActions: string[];
  /** Runner hotkeys in priority / UI order. */
  hotkeys: string[];
  /** winit key names (= DOM `KeyboardEvent.code` except Meta* → Super*), reserved included. */
  keyboardKeys: string[];
  gamepadInputs: GamepadInputInfo[];
  /** Never bindable (Escape). */
  reservedKeys: string[];
  /** Never bindable (guide). */
  reservedGamepadInputs: string[];
  /** Not bindable as pad hotkeys (back, start: Select+Start menu chord). */
  reservedPadHotkeyInputs: string[];
  /** Inputs per action (2). */
  maxBinds: number;
  /** section → action → default inputs. */
  defaults: Record<string, Record<string, string[]>>;
}

export interface AssignResult {
  /** Binding keys the input was taken from (toast "Z removed from Y"). */
  removedFrom: BindingKey[];
}

export interface CapturedInput {
  input: string;
  labelStyle: LabelStyle;
}

/** The UI waits this long for a pad press (backend caps at 10 s). */
export const GAMEPAD_CAPTURE_TIMEOUT_MS = 5000;

export const inputCommands = {
  getInputCatalog: () => invoke<InputCatalog>("get_input_catalog"),
  /** Binds `input` to `target`, stealing it from the device's other actions/hotkeys
   *  (one write). Rejects with kind "invalid" (unknown / reserved input, 3rd bind). */
  assignBinding: (scope: GameplayConfigScope, device: InputDevice, target: BindingKey, input: string) =>
    invoke<AssignResult>("assign_binding", { scope, device, target, input }),
  /** Sets one binding key (`[]` = unbound) or clears it with `null` ("Default"). */
  setBindingValue: (scope: GameplayConfigScope, key: BindingKey, value: string[] | null) =>
    invoke<void>("set_gameplay_config_value", { scope, key, value }),
  /** Removes every binding key of the scope (gameplay and resume settings stay). */
  resetControls: (scope: GameplayConfigScope) => invoke<void>("reset_controls", { scope }),
  /** Next pad press, or null on timeout / cancel. Rejects "busy" while a game runs. */
  captureGamepadInput: (timeoutMs: number = GAMEPAD_CAPTURE_TIMEOUT_MS) =>
    invoke<CapturedInput | null>("capture_gamepad_input", { timeoutMs }),
  cancelGamepadCapture: () => invoke<void>("cancel_gamepad_capture"),
  gamepadLabelStyle: () => invoke<LabelStyle>("gamepad_label_style"),
};
