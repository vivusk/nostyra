// The config layers one settings scope reads and writes on the web — the same
// files and order the desktop uses (`config/global.toml`, `system/<sys>.toml`,
// `core/<core layer>.toml` only when the web core is the desktop's libretro core
// (plan Q9: none for NES), `game/<sys>/<rom key> [ID].toml`). The global scope reads
// and writes the global layer alone, like the desktop's no-game Settings page.
import type { GameplayConfigScope } from "../../../api/tauri-commands-gameplay";
import { systemById } from "../catalog/web-systems";
import { getGame } from "../db/game-queries";
import type { DataDir } from "../fs/data-dir";
import { currentDataDir, dataFolderStatus } from "../fs/data-folder";
import { gameConfigPath } from "../player/game-data-paths";
import { splitRomPath } from "../scan/scan-paths";
import { commandError } from "../web-command-table";

export const GLOBAL_LAYER_PATH = "config/global.toml";

/** Texts of global, system, core, game (lowest first; null = missing file). */
export type LayerTexts = [string | null, string | null, string | null, string | null];

export interface ScopeLayers {
  data: DataDir;
  /** The game's system id; null = the global scope. */
  systemId: string | null;
  texts: LayerTexts;
  /** The layer a write to this scope edits. */
  editPath: string;
  /** Index of that layer in `texts`. */
  editIndex: 0 | 3;
}

/** Reads the layers of `scope` from the current Data folder. */
export async function readScopeLayers(scope: GameplayConfigScope): Promise<ScopeLayers> {
  const data = await currentDataDir();
  const global = await data.readText(GLOBAL_LAYER_PATH);
  if (scope === "global") {
    return { data, systemId: null, texts: [global, null, null, null], editPath: GLOBAL_LAYER_PATH, editIndex: 0 };
  }
  const game = await getGame(scope.gameId);
  if (!game) throw commandError("invalid", "This game is no longer in the library.");
  const system = await systemById(game.platformId);
  const relative = splitRomPath(game.romPath)?.relative ?? game.romPath;
  const key = { platformId: game.platformId, sha1: game.sha1, romFileName: relative.slice(relative.lastIndexOf("/") + 1) };
  const gamePath = await gameConfigPath(data, key);
  if (!gamePath) throw commandError("invalid", "This game has no usable content id; rescan the library.");
  const coreLayer = system?.coreLayerId ? `config/core/${system.coreLayerId}.toml` : null;
  const [systemText, coreText, gameText] = await Promise.all([
    data.readText(`config/system/${game.platformId}.toml`),
    coreLayer ? data.readText(coreLayer) : Promise.resolve(null),
    data.readText(gamePath),
  ]);
  return { data, systemId: game.platformId, texts: [global, systemText, coreText, gameText], editPath: gamePath, editIndex: 3 };
}

/** Writes the scope's edited layer (atomic). Refused while the picked Data folder
 *  needs "Reconnect": the edit would land in browser storage instead. */
export async function writeScopeLayer(layers: ScopeLayers, text: string): Promise<void> {
  if ((await dataFolderStatus()).needsAccess) {
    throw commandError("invalid", "Nostyra cannot reach your Data folder right now. Use “Reconnect” and try again.");
  }
  await layers.data.writeFile(layers.editPath, text);
}

/** Runs a wasm call; its error message becomes an `invalid` command error. */
export function wasmInvalid<T>(call: () => T): T {
  try {
    return call();
  } catch (error) {
    throw commandError("invalid", error instanceof Error ? error.message : String(error));
  }
}
