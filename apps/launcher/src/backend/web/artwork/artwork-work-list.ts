// The games an artwork pass fetches (src-tauri artwork_work_list.rs): all missing art
// (respecting the 7-day retry window for not-found / failed) or user-picked games
// (window ignored). Games of a system without a thumbnail repo are skipped.
import { gamesNeedingArtwork, uncachedArtworkKinds, type ArtworkKind } from "../db/artwork-queries";
import { getGame } from "../db/game-queries";
import { allSystems } from "../catalog/web-systems";
import type { GameArtRequest } from "./artwork-game-fetch";

/** Not-found / failed lookups are retried after this long (`ARTWORK_RETRY_S`). */
export const ARTWORK_RETRY_S = 7 * 24 * 60 * 60;
/** Upper bound per pass; a bigger library is finished by the next pass. */
const WORK_LIMIT = 20_000;
const KINDS: ArtworkKind[] = ["cover", "hero"];

async function repoOf(platformId: string): Promise<string | null> {
  return (await allSystems()).find((system) => system.id === platformId)?.thumbnailsRepo ?? null;
}

/** `missing_artwork_requests`: one request per game with its missing kinds, by id. */
export async function missingArtworkRequests(now: number): Promise<GameArtRequest[]> {
  const byGame = new Map<number, GameArtRequest>();
  for (const kind of KINDS) {
    for (const item of await gamesNeedingArtwork(kind, now - ARTWORK_RETRY_S, WORK_LIMIT)) {
      const repo = await repoOf(item.platformId);
      if (!repo) continue;
      const request = byGame.get(item.gameId) ?? {
        gameId: item.gameId,
        repo,
        noIntroName: item.datName,
        title: item.title,
        kinds: [],
      };
      request.kinds.push(kind);
      byGame.set(item.gameId, request);
    }
  }
  return [...byGame.values()].sort((a, b) => a.gameId - b.gameId);
}

/** `game_artwork_requests`: kinds without a cached file, retry window ignored; games
 *  without a No-Intro name have nothing to look up. */
export async function gameArtworkRequests(gameIds: number[]): Promise<GameArtRequest[]> {
  const requests: GameArtRequest[] = [];
  for (const gameId of gameIds) {
    const game = await getGame(gameId);
    const repo = game && (await repoOf(game.platformId));
    if (!game?.datName || !repo || requests.some((r) => r.gameId === gameId)) continue;
    const kinds = await uncachedArtworkKinds(gameId, KINDS);
    if (kinds.length > 0) requests.push({ gameId, repo, noIntroName: game.datName, title: game.title, kinds });
  }
  return requests;
}
