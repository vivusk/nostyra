// Controls editor on the web (desktop input_catalog_commands.rs +
// gamepad_capture_commands.rs): the shared catalog (hotkeys limited to the ones the
// browser player has), assign-with-steal and "Reset controls" through the shared
// Rust rules on the scope's TOML layer, and Gamepad API capture instead of SDL.
import type { GameplayConfigScope } from "../../../api/tauri-commands-gameplay";
import type { AssignResult, InputCatalog } from "../../../api/tauri-commands-input";
import { WEB_HOTKEYS } from "../../../lib/web-input-map";
import { readScopeLayers, wasmInvalid, writeScopeLayer } from "../config/config-scope";
import { loadWasm } from "../wasm/wasm-module";
import type { CommandTable } from "../web-command-table";
import { cancelGamepadCapture, captureGamepadInput, gamepadLabelStyle } from "./gamepad-capture";

let catalog: Promise<InputCatalog> | null = null;

/** `get_input_catalog`; hotkeys the browser has no use for are not offered. */
function inputCatalog(): Promise<InputCatalog> {
  catalog ??= loadWasm().then((wasm) => {
    const full = JSON.parse(wasm.input_catalog_json()) as InputCatalog;
    return { ...full, hotkeys: full.hotkeys.filter((hotkey) => WEB_HOTKEYS.includes(hotkey)) };
  });
  return catalog;
}

async function assignBinding(scope: GameplayConfigScope, device: string, target: string, input: string): Promise<AssignResult> {
  const layers = await readScopeLayers(scope);
  const wasm = await loadWasm();
  const [global, system, core, game] = layers.texts;
  const outcome = wasmInvalid(
    () => JSON.parse(wasm.config_assign_binding(layers.systemId, global, system, core, game, device, target, input)) as {
      text: string;
      removedFrom: string[];
    },
  );
  await writeScopeLayer(layers, outcome.text);
  return { removedFrom: outcome.removedFrom };
}

async function resetControls(scope: GameplayConfigScope): Promise<void> {
  const layers = await readScopeLayers(scope);
  const text = layers.texts[layers.editIndex];
  if (text === null) return; // no layer file: nothing to reset (none created)
  const wasm = await loadWasm();
  await writeScopeLayer(layers, wasmInvalid(() => wasm.config_reset_controls(text)));
}

export const controlsCommands: CommandTable = {
  get_input_catalog: () => inputCatalog(),
  assign_binding: ({ scope, device, target, input }) =>
    assignBinding(scope as GameplayConfigScope, String(device), String(target), String(input)),
  reset_controls: ({ scope }) => resetControls(scope as GameplayConfigScope),
  capture_gamepad_input: ({ timeoutMs }) => captureGamepadInput(Number(timeoutMs) || 0),
  cancel_gamepad_capture: async () => cancelGamepadCapture(),
  gamepad_label_style: async () => gamepadLabelStyle(),
};

