// Gameplay settings on the web (desktop gameplay_config_commands.rs): the same view
// (resolved / global / game values, bindings apart) and single-key writes, built by
// the shared Rust rules through wasm on the TOML texts of the Data folder. Unknown
// keys of a layer file survive every write, so the desktop sees an identical file.
import type { GameplayConfigScope, GameplayConfigView } from "../../../api/tauri-commands-gameplay";
import { loadWasm } from "../wasm/wasm-module";
import type { CommandTable } from "../web-command-table";
import { readScopeLayers, wasmInvalid, writeScopeLayer } from "./config-scope";

/** `get_gameplay_config`: a game's view (`gameId`), or the global layer's. */
export async function gameplayConfig(gameId: number | null): Promise<GameplayConfigView> {
  const layers = await readScopeLayers(gameId === null ? "global" : { gameId });
  const wasm = await loadWasm();
  const [global, system, core, game] = layers.texts;
  const { view, skipped } = JSON.parse(wasm.gameplay_settings_view(layers.systemId, global, system, core, game)) as {
    view: GameplayConfigView;
    skipped: string[];
  };
  for (const warning of skipped) console.warn(warning);
  return view;
}

/** `set_gameplay_config_value`: sets one known key, or clears it with `null`. */
async function setValue(scope: GameplayConfigScope, key: string, value: unknown): Promise<void> {
  const layers = await readScopeLayers(scope);
  const wasm = await loadWasm();
  const text = wasmInvalid(() =>
    wasm.config_set_known_value(layers.texts[layers.editIndex], key, JSON.stringify(value ?? null), scope !== "global"),
  );
  await writeScopeLayer(layers, text);
}

export const gameplayConfigCommands: CommandTable = {
  get_gameplay_config: ({ gameId }) => gameplayConfig(gameId == null ? null : Number(gameId)),
  set_gameplay_config_value: ({ scope, key, value }) => setValue(scope as GameplayConfigScope, String(key), value),
};
