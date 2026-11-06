// Library reads of the web library DB, mirroring crates/storage-db/src/game_queries.rs
// (list, summary, one game, platform counts) and `list_genres`
// (game_metadata_repository.rs). Rows come back as the launcher's `Game` shape with
// art as asset refs (desktop: absolute paths).
import type { Game, GameListQuery, LibrarySummary } from "../../../api/tauri-commands";
import { artworkRef } from "../artwork/artwork-refs";
import { dbQuery, dbQueryOne } from "./db-client";
import type { SqlRow, SqlValue } from "./db-protocol";
import { GAME_SELECT, RECENTLY_PLAYED_WINDOW_S, listGamesSql, searchPattern } from "./game-list-sql";

/** `game_from_row` → the launcher's `Game` (+ `serial`, as the desktop sends it). */
export function gameFromRow(row: SqlRow): Game & { serial: string | null } {
  return {
    id: Number(row.id),
    platformId: String(row.platform_id),
    folderId: row.folder_id === null ? null : Number(row.folder_id),
    romPath: String(row.rom_path),
    fileSize: Number(row.file_size),
    crc32: String(row.crc32),
    sha1: String(row.sha1),
    title: String(row.title),
    region: row.region as string | null,
    datName: row.dat_name as string | null,
    addedAt: Number(row.added_at),
    missing: Boolean(row.missing),
    totalPlayS: Number(row.total_play_s),
    lastPlayedAt: row.last_played_at === null ? null : Number(row.last_played_at),
    genre: row.genre as string | null,
    releaseYear: row.release_year === null ? null : Number(row.release_year),
    developer: row.developer as string | null,
    favorite: Boolean(row.favorite),
    coverPath: artworkRef(row.cover_path as string | null),
    heroPath: artworkRef(row.hero_path as string | null),
    coverCheckedAt: row.cover_checked_at === null ? null : Number(row.cover_checked_at),
    serial: row.serial as string | null,
  };
}

/** `list_games`: visible games, filtered and sorted. */
export async function listGames(query: GameListQuery): Promise<Game[]> {
  const genre = query.genre?.trim() || null;
  const params: SqlValue[] = [searchPattern(query.search), genre, query.platform ?? null, query.limit ?? null];
  // A parameter missing from the statement is an error: bind the window only when used.
  if (query.filter === "recently-played") params.push(RECENTLY_PLAYED_WINDOW_S);
  return (await dbQuery(listGamesSql(query.filter, query.sort), params)).map(gameFromRow);
}

/** `library_summary`: counts for the library header (visible games only). */
export async function librarySummary(): Promise<LibrarySummary> {
  const row = await dbQueryOne<{ total: number; matched: number; missing: number }>(
    `SELECT COUNT(*) AS total, COUNT(dat_name) AS matched, COALESCE(SUM(missing), 0) AS missing
     FROM games WHERE folder_id IS NOT NULL`,
  );
  const total = row?.total ?? 0;
  const matched = row?.matched ?? 0;
  return { total, matched, needsReview: total - matched, missing: row?.missing ?? 0 };
}

/** `get_game`: one game by id (hidden rows included). */
export async function getGame(gameId: number): Promise<(Game & { serial: string | null }) | null> {
  const row = await dbQueryOne(`${GAME_SELECT} WHERE g.id = ?`, [gameId]);
  return row ? gameFromRow(row) : null;
}

/** `library_platform_counts`: visible games per platform id. */
export async function libraryPlatformCounts(): Promise<{ platformId: string; count: number }[]> {
  const rows = await dbQuery<{ platform_id: string; count: number }>(
    `SELECT platform_id, COUNT(*) AS count FROM games WHERE folder_id IS NOT NULL
     GROUP BY platform_id ORDER BY platform_id`,
  );
  return rows.map((row) => ({ platformId: row.platform_id, count: row.count }));
}

/** `list_genres`: distinct genres of visible games, A–Z. */
export async function listGenres(): Promise<string[]> {
  const rows = await dbQuery<{ genre: string }>(
    `SELECT DISTINCT m.genre AS genre FROM game_metadata m JOIN games g ON g.id = m.game_id
     WHERE g.folder_id IS NOT NULL AND m.genre IS NOT NULL ORDER BY m.genre`,
  );
  return rows.map((row) => row.genre);
}

/** `visible_missing_game_ids`: the Library "Missing" list. */
export async function visibleMissingGameIds(): Promise<number[]> {
  const rows = await dbQuery<{ id: number }>("SELECT id FROM games WHERE missing = 1 AND folder_id IS NOT NULL ORDER BY id");
  return rows.map((row) => row.id);
}
