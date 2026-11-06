// Metadata, favorites, play stats and missing-game removal in the web library DB,
// mirroring crates/storage-db/src/{game_metadata_repository, user_metadata_edits,
// play_session_repository, game_removal}.rs.
import type { GameMetadata, PlayStats, UserMetadataEdit } from "../../../api/tauri-commands-artwork";
import type { ScannedMetadata } from "../scan/scan-protocol";
import { dbBatch, dbQueryOne } from "./db-client";
import { cleanUserMetadata } from "./user-metadata-edit";

/** `get_game_metadata`: the stored metadata row of a game, if any. */
export async function getGameMetadata(gameId: number): Promise<GameMetadata | null> {
  const row = await dbQueryOne<Record<string, string | number | null>>(
    `SELECT title_override, developer, publisher, genre, release_year, players, description, user_edited, updated_at
     FROM game_metadata WHERE game_id = ?`,
    [gameId],
  );
  if (!row) return null;
  return {
    titleOverride: row.title_override as string | null,
    developer: row.developer as string | null,
    publisher: row.publisher as string | null,
    genre: row.genre as string | null,
    releaseYear: row.release_year as number | null,
    players: row.players as number | null,
    description: row.description as string | null,
    userEdited: Boolean(row.user_edited),
    updatedAt: Number(row.updated_at),
  };
}

/** `game_play_stats`: total play time, last session start, session count. */
export async function gamePlayStats(gameId: number): Promise<PlayStats> {
  const row = await dbQueryOne<{ total: number; last: number | null; count: number }>(
    "SELECT COALESCE(SUM(duration_s), 0) AS total, MAX(started_at) AS last, COUNT(*) AS count FROM play_sessions WHERE game_id = ?",
    [gameId],
  );
  return { totalPlayS: row?.total ?? 0, lastPlayedAt: row?.last ?? null, sessionCount: row?.count ?? 0 };
}

/** `set_favorite`: returns whether the game exists. */
export async function setFavorite(gameId: number, favorite: boolean): Promise<boolean> {
  const [rows] = await dbBatch([{ sql: "UPDATE games SET favorite = ? WHERE id = ? RETURNING id", params: [favorite ? 1 : 0, gameId] }]);
  return rows.length > 0;
}

/** `update_user_metadata`: saves the user's values; the row is frozen until reset.
 *  Throws `InvalidMetadataInput` for out-of-range numbers. */
export async function updateUserMetadata(gameId: number, edit: UserMetadataEdit, sourceVersion: string, now: number): Promise<void> {
  const v = cleanUserMetadata(edit);
  await dbBatch([
    {
      sql: `INSERT INTO game_metadata (game_id, title_override, title_override_search, developer, publisher, genre,
          release_year, players, description, source_version, user_edited, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)
        ON CONFLICT(game_id) DO UPDATE SET title_override = excluded.title_override,
          title_override_search = excluded.title_override_search, developer = excluded.developer,
          publisher = excluded.publisher, genre = excluded.genre, release_year = excluded.release_year,
          players = excluded.players, description = excluded.description, user_edited = 1,
          updated_at = excluded.updated_at`,
      params: [gameId, v.title, v.titleSearch, v.developer, v.publisher, v.genre, v.releaseYear, v.players, v.description, sourceVersion, now],
    },
  ]);
}

/** `delete_metadata` + `upsert_from_index` ("Reset to database values") in one transaction. */
export async function resetMetadata(gameId: number, indexed: ScannedMetadata | null, now: number): Promise<void> {
  await dbBatch([
    { sql: "DELETE FROM game_metadata WHERE game_id = ?", params: [gameId] },
    ...(indexed
      ? [
          {
            sql: `INSERT INTO game_metadata (game_id, developer, publisher, genre, release_year, players, source_version, updated_at)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
            params: [gameId, indexed.developer, indexed.publisher, indexed.genre, indexed.releaseYear, indexed.players, indexed.sourceVersion, now],
          },
        ]
      : []),
  ]);
}

/** `delete_missing_game`: deletes a game whose ROM is missing (sessions, metadata and
 *  artwork rows cascade); false when unknown or present again. */
export async function deleteMissingGame(gameId: number): Promise<boolean> {
  const [rows] = await dbBatch([{ sql: "DELETE FROM games WHERE id = ? AND missing = 1 RETURNING id", params: [gameId] }]);
  return rows.length > 0;
}
