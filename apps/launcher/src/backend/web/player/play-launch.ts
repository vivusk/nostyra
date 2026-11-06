// Everything a web game start needs, gathered before the player opens (desktop
// `play_game` + runner start-up): the game row, its browser core, the Data folder
// save folder and config, the content File, BIOS files, the SRAM to inject (the
// D7 rule) and the state to load. Errors here reject Play with a toast; nothing is
// running yet.
import type { StateSlotId } from "../../../api/tauri-commands-gameplay";
import { systemById, type WebSystem } from "../catalog/web-systems";
import { getGame } from "../db/game-queries";
import type { DataDir } from "../fs/data-dir";
import { currentDataDir, dataFolderStatus } from "../fs/data-folder";
import { commandError } from "../web-command-table";
import { codeFromKeyName } from "../../../lib/keyboard-event-code";
import { playerInput } from "../../../lib/web-input-map";
import { scanBios } from "./bios-files";
import { prepareContent } from "./content-prepare";
import { dataFileNames, gameSavesDir, type DataFileNames, type GameKey } from "./game-data-paths";
import { resolveGameConfig, resumeSettings } from "./game-config";
import { EMULATORJS_VERSION, playerSavePaths, type PlayerBoot } from "./player-messages";
import { openRomFolder } from "./rom-files";
import { sramToLoad, type SramFolderFacts } from "./sram-rule";
import { readWebState } from "./web-state-slots";

export interface PlayLaunch {
  gameId: number;
  romPath: string;
  sha1: string;
  system: WebSystem;
  ejsCore: string;
  /** The core's save folder inside EmulatorJS (`/data/saves/<saveDir>/`). */
  saveDir: string;
  data: DataDir;
  /** `saves/<system>/<entry>`. */
  savesDir: string;
  names: DataFileNames;
  facts: SramFolderFacts;
  loaded: { sram: Uint8Array | null; rtc: Uint8Array | null };
  patchCrc32: number | null;
  autoSave: boolean;
  boot: PlayerBoot;
}

/** `play_game`'s checks, in the desktop's order and wording. */
async function playableGame(gameId: number) {
  const game = await getGame(gameId);
  if (!game) throw commandError("invalid", "This game is no longer in the library.");
  if (game.missing) throw commandError("invalid", "The ROM file was not found. Rescan your library.");
  const system = await systemById(game.platformId);
  if (!system?.webCore) {
    throw commandError("unsupported", `${system?.displayName ?? game.platformId} games need the desktop app.`);
  }
  if ((await dataFolderStatus()).needsAccess) {
    throw commandError("invalid", "Nostyra cannot reach your Data folder right now. Use “Reconnect” and press Play again.");
  }
  return { game, system, webCore: system.webCore };
}

async function readBytes(data: DataDir, path: string): Promise<Uint8Array | null> {
  const file = await data.readFile(path);
  return file ? new Uint8Array(await file.arrayBuffer()) : null;
}

export async function preparePlay(gameId: number, loadState: StateSlotId | null): Promise<PlayLaunch> {
  const { game, system, webCore } = await playableGame(gameId);
  const folder = await openRomFolder(game.romPath);
  const data = await currentDataDir();
  const names = await dataFileNames();
  const key: GameKey = {
    platformId: game.platformId,
    sha1: game.sha1,
    romFileName: folder.relative.slice(folder.relative.lastIndexOf("/") + 1),
  };
  const savesDir = await gameSavesDir(data, key);
  const config = await resolveGameConfig(data, system, key);
  const resume = resumeSettings(config);
  const content = await prepareContent(folder, system);

  const sramFile = await data.readFile(`${savesDir}/${names.sram}`);
  const facts: SramFolderFacts = {
    webSramExists: await data.exists(`${savesDir}/${names.webSram}`),
    sramLength: sramFile ? sramFile.size : null,
  };
  const source = sramToLoad(facts);
  const sram = source ? await readBytes(data, `${savesDir}/${source === "webSram" ? names.webSram : names.sram}`) : null;
  const rtc = await readBytes(data, `${savesDir}/${names.rtc}`);
  const paths = playerSavePaths(webCore.saveDir);
  const saves: Record<string, Uint8Array> = {};
  if (sram) saves[paths.sram] = sram;
  if (rtc) saves[paths.rtc] = rtc;

  const bios: Record<string, Uint8Array> = {};
  for (const file of await scanBios(data, system.firmware)) {
    if (file.valid) bios[file.fileName] = new Uint8Array(await file.file.arrayBuffer());
  }

  let state: Uint8Array | null = null;
  let notice: string | null = null;
  if (loadState !== null) {
    const read = await readWebState(data, savesDir, loadState, webCore.ejsCore);
    if ("bytes" in read) state = read.bytes;
    else notice = `${read.refusal} Started fresh.`;
  }
  return {
    gameId,
    romPath: game.romPath,
    sha1: game.sha1,
    system,
    ejsCore: webCore.ejsCore,
    saveDir: webCore.saveDir,
    data,
    savesDir,
    names,
    facts,
    loaded: { sram, rtc },
    patchCrc32: content.patchCrc32,
    autoSave: resume.autoSave,
    boot: {
      core: webCore.ejsCore,
      rom: content.rom,
      pathToData: `${import.meta.env.BASE_URL}emulatorjs/${EMULATORJS_VERSION}/`,
      saves,
      bios,
      loadState: state,
      notice,
      input: playerInput(config, system.padActions, codeFromKeyName),
    },
  };
}
