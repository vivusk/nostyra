// Where one game's shared files live in the Data folder, found exactly like the
// desktop (`ipc_protocol::{find_game_entry, game_entry_name}` through wasm): the
// save folder `saves/<system>/<rom key> [ID]` and the game config layer
// `config/game/<system>/<rom key> [ID].toml`. Lookup is by the ` [ID]` suffix
// (content sha1), so a renamed ROM or a desktop-made folder is found as is.
import type { DataDir } from "../fs/data-dir";
import { loadWasm } from "../wasm/wasm-module";
import { entryNameFor } from "./web-file-names";

/** The facts the paths depend on (`games` row). */
export interface GameKey {
  platformId: string;
  sha1: string;
  /** File name of the game's main file (rom key source: the desktop's `--rom`). */
  romFileName: string;
}

const GAME_CONFIG_EXT = ".toml";

/** Name of the game's entry in `parent` (existing one first, else a new name);
 *  null when the stored sha1 is unusable. */
async function entryName(data: DataDir, parent: string, game: GameKey, ext: string): Promise<string | null> {
  const wasm = await loadWasm();
  const names = (await data.list(parent)).map((entry) => entry.name);
  return entryNameFor(names, game.sha1, game.romFileName, ext, { find: wasm.find_entry_name, create: wasm.new_entry_name });
}

/** `saves/<system>/<entry>` (may not exist yet; writes create it). */
export async function gameSavesDir(data: DataDir, game: GameKey): Promise<string> {
  const parent = `saves/${game.platformId}`;
  const name = await entryName(data, parent, game, "");
  if (!name) throw new Error("This game has no usable content id; rescan the library.");
  return `${parent}/${name}`;
}

/** `config/game/<system>/<entry>.toml`, or null without a usable content id. */
export async function gameConfigPath(data: DataDir, game: GameKey): Promise<string | null> {
  const parent = `config/game/${game.platformId}`;
  const name = await entryName(data, parent, game, GAME_CONFIG_EXT);
  return name ? `${parent}/${name}` : null;
}

/** Per-game file names shared with the desktop (`save.srm`, `save.web.srm`, `rtc.bin`). */
export interface DataFileNames {
  sram: string;
  webSram: string;
  rtc: string;
}

let fileNames: Promise<DataFileNames> | null = null;

export function dataFileNames(): Promise<DataFileNames> {
  fileNames ??= loadWasm().then((wasm) => JSON.parse(wasm.data_file_names()) as DataFileNames);
  return fileNames;
}
