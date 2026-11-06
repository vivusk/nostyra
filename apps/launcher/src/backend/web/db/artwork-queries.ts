// Artwork lookup rows of the web library DB, mirroring
// crates/storage-db/src/artwork_repository.rs: one row per (game, kind) for cached /
// not-found / failed; transient failures write nothing (retried by the next job).
import type { ArtworkStatus } from "../../../api/tauri-commands-artwork";
import { dbBatch, dbQuery } from "./db-client";

export type ArtworkKind = "cover" | "hero";

/** One `artwork` row (camelCase like the desktop's serialised `ArtworkRecord`). */
export interface ArtworkRow {
  gameId: number;
  kind: ArtworkKind;
  status: ArtworkStatus;
  /** Relative to the artwork cache; null unless cached. */
  localPath: string | null;
  source: string;
  sourceName: string | null;
  checkedAt: number;
}

/** A game to fetch art for (`ArtworkWorkItem`). */
export interface ArtworkWorkItem {
  gameId: number;
  platformId: string;
  datName: string;
  title: string;
}

/** `get_artwork`: all rows of a game. */
export async function getArtwork(gameId: number): Promise<ArtworkRow[]> {
  const rows = await dbQuery<Record<string, string | number | null>>(
    "SELECT game_id, kind, status, local_path, source, source_name, checked_at FROM artwork WHERE game_id = ? ORDER BY kind",
    [gameId],
  );
  return rows.map((r) => ({
    gameId: Number(r.game_id),
    kind: r.kind as ArtworkKind,
    status: (r.status === "cached" || r.status === "not-found" ? r.status : "failed") as ArtworkStatus,
    localPath: r.local_path as string | null,
    source: String(r.source),
    sourceName: r.source_name as string | null,
    checkedAt: Number(r.checked_at),
  }));
}

/** `upsert_artwork`: inserts or replaces the row of `(gameId, kind)`. */
export async function upsertArtwork(row: ArtworkRow): Promise<void> {
  await dbBatch([
    {
      sql: `INSERT INTO artwork (game_id, kind, status, local_path, source, source_name, checked_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(game_id, kind) DO UPDATE SET status = excluded.status, local_path = excluded.local_path,
          source = excluded.source, source_name = excluded.source_name, checked_at = excluded.checked_at`,
      params: [row.gameId, row.kind, row.status, row.localPath, row.source, row.sourceName, row.checkedAt],
    },
  ]);
}

/** `games_needing_artwork`: No-Intro-matched, visible, present games never looked up
 *  for `kind` or whose non-cached result is older than `retryBefore`. Oldest first. */
export async function gamesNeedingArtwork(kind: ArtworkKind, retryBefore: number, limit: number): Promise<ArtworkWorkItem[]> {
  const rows = await dbQuery<{ id: number; platform_id: string; dat_name: string; title: string }>(
    `SELECT g.id, g.platform_id, g.dat_name, g.title FROM games g
     LEFT JOIN artwork a ON a.game_id = g.id AND a.kind = ?1
     WHERE g.dat_name IS NOT NULL AND g.folder_id IS NOT NULL AND g.missing = 0
       AND (a.game_id IS NULL OR (a.status <> 'cached' AND a.checked_at < ?2))
     ORDER BY g.id LIMIT ?3`,
    [kind, retryBefore, limit],
  );
  return rows.map((r) => ({ gameId: r.id, platformId: r.platform_id, datName: r.dat_name, title: r.title }));
}

/** `uncached_artwork_kinds`: kinds among `kinds` the game has no cached file for. */
export async function uncachedArtworkKinds(gameId: number, kinds: readonly ArtworkKind[]): Promise<ArtworkKind[]> {
  const cached = await dbQuery<{ kind: ArtworkKind }>("SELECT kind FROM artwork WHERE game_id = ? AND status = 'cached'", [gameId]);
  return kinds.filter((kind) => !cached.some((row) => row.kind === kind));
}

/** `clear_artwork`: deletes every row (Clear cache; the caller removes the files). */
export async function clearArtwork(): Promise<void> {
  await dbBatch([{ sql: "DELETE FROM artwork" }]);
}
