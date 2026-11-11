// Game Detail commands of the web backend (src-tauri game_detail_commands.rs):
// detail view, favorite, metadata edit / reset, genres. "Reset to database values"
// re-applies the vendored metadat through the wasm module, as the desktop does.
import type { GameDetail, UserMetadataEdit } from "../../../api/tauri-commands-artwork";
import { systemDat } from "../catalog/system-dat";
import { systemById } from "../catalog/web-systems";
import { getArtwork } from "../db/artwork-queries";
import { getGame, listGenres } from "../db/game-queries";
import { gamePlayStats, getGameMetadata, resetMetadata, setFavorite, updateUserMetadata } from "../db/game-metadata-queries";
import { InvalidMetadataInput } from "../db/user-metadata-edit";
import type { ScannedMetadata } from "../scan/scan-protocol";
import { EMULATORJS_VERSION } from "../player/player-messages";
import { splitRomPath } from "../scan/scan-paths";
import { loadWasm } from "../wasm/wasm-module";
import { commandError, type CommandTable } from "../web-command-table";

const unixNow = () => Math.floor(Date.now() / 1000);
const notFound = () => commandError("invalid", "This game is no longer in the library.");

/** `detail_view`: game + metadata + stats + display extras; null = unknown id. */
async function detailView(gameId: number): Promise<GameDetail | null> {
  const game = await getGame(gameId);
  if (!game) return null;
  const relative = splitRomPath(game.romPath)?.relative ?? game.romPath;
  const system = await systemById(game.platformId);
  const webCore = system?.webCore;
  return {
    ...game,
    metadata: await getGameMetadata(gameId),
    stats: await gamePlayStats(gameId),
    romFileName: relative.slice(relative.lastIndexOf("/") + 1),
    coreName: webCore ? `${webCore.ejsCore} (EmulatorJS)` : null,
    coreVersion: webCore ? EMULATORJS_VERSION : null,
    artwork: await getArtwork(gameId),
    // Desktop facts: the web shows Play online with a "desktop app only" notice.
    netplay: system
      ? { support: system.desktopNetplay, maxPlayers: system.desktopNetplayPlayers, offline: false }
      : undefined,
  };
}

async function existingDetail(gameId: number): Promise<GameDetail> {
  const detail = await detailView(gameId);
  if (!detail) throw notFound();
  return detail;
}

export const detailCommands: CommandTable = {
  get_game_detail: ({ gameId }) => detailView(Number(gameId)),
  set_favorite: async ({ gameId, favorite }) => {
    if (!(await setFavorite(Number(gameId), Boolean(favorite)))) throw notFound();
  },
  update_game_metadata: async ({ gameId, edit }) => {
    const id = Number(gameId);
    if (!(await getGame(id))) throw notFound();
    try {
      const version = (await loadWasm()).metadata_source_version();
      await updateUserMetadata(id, edit as UserMetadataEdit, version, unixNow());
    } catch (error) {
      if (error instanceof InvalidMetadataInput) throw commandError("invalid", error.message);
      throw error;
    }
    return existingDetail(id);
  },
  /** Deletes the row, then re-applies the vendored metadat when the game is
   *  No-Intro-matched and known to it (else the fields stay empty). */
  reset_game_metadata: async ({ gameId }) => {
    const id = Number(gameId);
    const game = await getGame(id);
    if (!game) throw notFound();
    let indexed: ScannedMetadata | null = null;
    const system = game.datName ? await systemById(game.platformId) : null;
    if (system) {
      const dat = await systemDat(system);
      indexed = JSON.parse(dat.metadata_for_game(game.datName, game.crc32, game.serial)) as ScannedMetadata | null;
    }
    await resetMetadata(id, indexed, unixNow());
    return existingDetail(id);
  },
  list_genres: () => listGenres(),
};
