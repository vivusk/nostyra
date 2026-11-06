// Home data and missing-game removal on the web (src-tauri home_commands.rs,
// missing_game_removal_commands.rs). Unlike the desktop, removing a missing game
// deletes only the web library rows and its cached art: saves and config live in
// the shared Data folder (maybe the desktop app's) and are never touched here.
import type { GameFilter, GameListQuery, GameSort } from "../../../api/tauri-commands";
import type { HomeData } from "../../../api/tauri-commands-artwork";
import { removeArtworkFiles } from "../artwork/artwork-store";
import { getArtwork } from "../db/artwork-queries";
import { getGame, listGames, visibleMissingGameIds } from "../db/game-queries";
import { deleteMissingGame } from "../db/game-metadata-queries";
import { isScanning } from "../scan/scan-command";
import { commandError, type CommandTable } from "../web-command-table";

const CONTINUE_LIMIT = 4;
const RECENTLY_ADDED_LIMIT = 6;
/** A few favorites so the hero selector can pick one not already listed. */
const FAVORITES_LIMIT = 4;

const query = (sort: GameSort, filter: GameFilter, limit: number): GameListQuery => ({ sort, filter, limit });

/** `home_lists`: played games (most recent first), newest added, favorites. */
async function homeData(): Promise<HomeData> {
  const [continuePlaying, recentlyAdded, favorites] = await Promise.all([
    listGames(query("recently-played", "played", CONTINUE_LIMIT)),
    listGames(query("recently-added", "all", RECENTLY_ADDED_LIMIT)),
    listGames(query("recently-played", "favorites", FAVORITES_LIMIT)),
  ]);
  return { continuePlaying, recentlyAdded, favorites };
}

/** Removes `ids` whose ROM is missing; `strict` = one game the user picked (anything
 *  else is an error). Refused during a scan (it could restore a row mid-way). */
async function removeMissingGames(ids: number[], strict: boolean): Promise<number> {
  if (isScanning()) throw commandError("scan", "Wait for the scan to finish (or cancel it) before removing games.");
  let removed = 0;
  for (const gameId of ids) {
    const game = await getGame(gameId);
    if (!game?.missing) {
      if (strict) throw commandError("invalid", "Only games whose ROM file is missing can be removed.");
      continue;
    }
    const art = (await getArtwork(gameId)).flatMap((row) => (row.localPath ? [row.localPath] : []));
    await removeArtworkFiles(art);
    if (await deleteMissingGame(gameId)) removed += 1;
  }
  return removed;
}

export const homeCommands: CommandTable = {
  get_home_data: () => homeData(),
  remove_missing_game: async ({ gameId }) => {
    await removeMissingGames([Number(gameId)], true);
  },
  remove_all_missing_games: async () => removeMissingGames(await visibleMissingGameIds(), false),
};
