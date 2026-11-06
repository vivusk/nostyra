// Save states and the resume point on the web (src-tauri save_state_commands.rs,
// resume_commands.rs, resume_point.rs): the web's own `web-*` states in the game's
// save folder, the same result shapes and warnings. Achievement hardcore does not
// exist on the web.
import type { ResumeInfo, ResumeWarning, StateSlotId } from "../../../api/tauri-commands-gameplay";
import { systemById, type WebSystem } from "../catalog/web-systems";
import { getGame } from "../db/game-queries";
import type { DataDir } from "../fs/data-dir";
import { currentDataDir } from "../fs/data-folder";
import { ensureIdle } from "../player/active-session";
import { readPatchChain } from "../player/content-prepare";
import { crc32Update } from "../player/crc32-stream";
import { dataFileNames, gameSavesDir, type GameKey } from "../player/game-data-paths";
import { resolveGameConfig, resumeSettings } from "../player/game-config";
import { openRomFolder } from "../player/rom-files";
import { deleteWebState, listWebStates, readWebStateSidecar, webStateFiles } from "../player/web-state-slots";
import { splitRomPath } from "../scan/scan-paths";
import { commandError, type CommandTable } from "../web-command-table";

interface Target {
  game: NonNullable<Awaited<ReturnType<typeof getGame>>>;
  system: WebSystem;
  ejsCore: string;
  data: DataDir;
  key: GameKey;
}

/** The game's data facts; null for a missing ROM (it then has no states, as on the desktop). */
async function target(gameId: number): Promise<Target | null> {
  const game = await getGame(gameId);
  if (!game) throw commandError("invalid", "This game is no longer in the library.");
  const system = await systemById(game.platformId);
  if (game.missing || !system?.webCore) return null;
  const relative = splitRomPath(game.romPath)?.relative ?? game.romPath;
  const key = { platformId: game.platformId, sha1: game.sha1, romFileName: relative.slice(relative.lastIndexOf("/") + 1) };
  return { game, system, ejsCore: system.webCore.ejsCore, data: await currentDataDir(), key };
}

/** CRC of the SRAM the player would load now (null = none). */
async function currentSramCrc(data: DataDir, dir: string): Promise<number | null> {
  const names = await dataFileNames();
  const file = (await data.readFile(`${dir}/${names.webSram}`)) ?? (await data.readFile(`${dir}/${names.sram}`));
  return file ? crc32Update(0, new Uint8Array(await file.arrayBuffer())) : null;
}

/** Patch chain CRC now; undefined when the ROM folder cannot be read without a prompt. */
async function currentPatchCrc(romPath: string): Promise<number | null | undefined> {
  try {
    return (await readPatchChain(await openRomFolder(romPath))).patchCrc32;
  } catch {
    return undefined;
  }
}

async function resumeInfo(gameId: number): Promise<ResumeInfo> {
  const found = await target(gameId);
  const settings = found
    ? resumeSettings(await resolveGameConfig(found.data, found.system, found.key))
    : { mode: "ask" as const };
  const none: ResumeInfo = { mode: settings.mode, available: false, createdAt: null, thumbnailPath: null, warnings: [], incompatibleReason: null, hardcore: false };
  if (!found) return none;
  const dir = await gameSavesDir(found.data, found.key);
  const auto = (await listWebStates(found.data, dir, found.ejsCore)).find((slot) => slot.slot === "auto");
  const sidecar = auto ? await readWebStateSidecar(found.data, dir, "auto") : null;
  if (!auto || !sidecar) return none;
  const warnings: ResumeWarning[] = [];
  const sram = await currentSramCrc(found.data, dir);
  if (sidecar.sram_crc32 !== undefined && sram !== null && sidecar.sram_crc32 !== sram) warnings.push("sram-newer");
  const patch = await currentPatchCrc(found.game.romPath);
  if (patch !== undefined && (sidecar.rom_patch_crc32 ?? null) !== patch) warnings.push("patch-changed");
  if (auto.incompatibleReason) warnings.push("state-incompatible");
  return { ...none, available: true, createdAt: auto.createdAt, thumbnailPath: auto.thumbnailPath, warnings, incompatibleReason: auto.incompatibleReason };
}

export const stateCommands: CommandTable = {
  list_save_states: async ({ gameId }) => {
    const found = await target(Number(gameId));
    if (!found) return [];
    return listWebStates(found.data, await gameSavesDir(found.data, found.key), found.ejsCore);
  },
  delete_save_state: async ({ gameId, slot }) => {
    const found = await target(Number(gameId));
    if (!found) throw commandError("invalid", "The ROM file was not found. Rescan your library.");
    const id = slot as StateSlotId;
    const dir = await gameSavesDir(found.data, found.key);
    await webStateFiles(dir, id).catch((error: Error) => {
      throw commandError("invalid", error.message);
    });
    ensureIdle(found.game.id);
    await deleteWebState(found.data, dir, id).catch((error) => {
      throw commandError("storage", `Cannot delete the save: ${String(error)}`);
    });
  },
  get_resume_info: ({ gameId }) => resumeInfo(Number(gameId)),
};
