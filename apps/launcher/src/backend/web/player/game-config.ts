// A game's effective config on the web: the same four TOML layers the desktop
// runner reads (global, system, core — only when the web core is the desktop's
// libretro core, plan Q9 — and the game layer), merged by the shared Rust rules
// through wasm (`config_resolve`). Values are keyed by dotted name ("resume.mode").
import type { WebSystem } from "../catalog/web-systems";
import type { DataDir } from "../fs/data-dir";
import { loadWasm } from "../wasm/wasm-module";
import { gameConfigPath, type GameKey } from "./game-data-paths";

export type ConfigValues = Record<string, unknown>;

/** Resolved known values for `game` (built-in defaults where no layer sets a key). */
export async function resolveGameConfig(data: DataDir, system: WebSystem, game: GameKey): Promise<ConfigValues> {
  const gamePath = await gameConfigPath(data, game);
  const [global, systemLayer, core, gameLayer] = await Promise.all([
    data.readText("config/global.toml"),
    data.readText(`config/system/${system.id}.toml`),
    system.coreLayerId ? data.readText(`config/core/${system.coreLayerId}.toml`) : Promise.resolve(null),
    gamePath ? data.readText(gamePath) : Promise.resolve(null),
  ]);
  const wasm = await loadWasm();
  const view = JSON.parse(wasm.config_resolve(system.id, global, systemLayer, core, gameLayer)) as {
    values: ConfigValues;
    warnings: string[];
  };
  for (const warning of view.warnings) console.warn(warning);
  return view.values;
}

/** What Play does with a resume point, and whether exits write one. */
export interface ResumeSettings {
  mode: "ask" | "always" | "never";
  autoSave: boolean;
}

export function resumeSettings(values: ConfigValues): ResumeSettings {
  const mode = values["resume.mode"];
  return {
    mode: mode === "always" || mode === "never" ? mode : "ask",
    autoSave: values["resume.auto_save"] !== false,
  };
}
