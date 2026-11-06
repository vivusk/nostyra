// Scan writes of the web library DB, mirroring crates/storage-db/src/game_scan_writes.rs:
// batched upsert with sha1 relink, missing marks, rescan fingerprints. The worker
// RPC runs a batch as fixed statements, so the desktop's per-row branches become
// guarded statements (update → relink if no row has the path → insert if still none).
import type { ScannedRow } from "../scan/scan-protocol";
import type { KnownFile } from "../scan/scan-paths";
import { dbBatch, dbQuery } from "./db-client";
import type { SqlStatement, SqlValue } from "./db-protocol";

/** `SET_SCANNED_COLUMNS`: `?1` = rom_path, `?2..?12` = the rest. */
const SET_SCANNED_COLUMNS = `platform_id = ?2, folder_id = ?3, file_size = ?4, file_mtime_ns = ?5,
  crc32 = ?6, sha1 = ?7, title = ?8, title_search = ?9, region = ?10, dat_name = ?11, serial = ?12, missing = 0`;

const UPDATE_BY_PATH = `UPDATE games SET ${SET_SCANNED_COLUMNS} WHERE rom_path = ?1 RETURNING id`;
const RELINK = `UPDATE games SET rom_path = ?1, ${SET_SCANNED_COLUMNS}
  WHERE id = (SELECT id FROM games WHERE sha1 = ?7 AND platform_id = ?2 AND missing = 1
    AND NOT EXISTS (SELECT 1 FROM games WHERE rom_path = ?1) ORDER BY id LIMIT 1)
  RETURNING id`;
const INSERT = `INSERT INTO games (rom_path, platform_id, folder_id, file_size, file_mtime_ns, crc32, sha1,
    title, title_search, region, dat_name, serial, added_at)
  SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
  WHERE NOT EXISTS (SELECT 1 FROM games WHERE rom_path = ?1) RETURNING id`;
/** `UPSERT_FROM_INDEX_SQL` keyed by rom path (user-edited rows are kept). */
const WRITE_METADATA = `INSERT INTO game_metadata
    (game_id, developer, publisher, genre, release_year, players, source_version, updated_at)
  SELECT id, ?2, ?3, ?4, ?5, ?6, ?7, ?8 FROM games WHERE rom_path = ?1
  ON CONFLICT(game_id) DO UPDATE SET developer = excluded.developer, publisher = excluded.publisher,
    genre = excluded.genre, release_year = excluded.release_year, players = excluded.players,
    source_version = excluded.source_version, updated_at = excluded.updated_at
  WHERE game_metadata.user_edited = 0`;
const DROP_METADATA = `DELETE FROM game_metadata WHERE user_edited = 0
  AND game_id = (SELECT id FROM games WHERE rom_path = ?1)`;

export interface UpsertOutcome {
  inserted: number;
  updated: number;
  relinked: number;
}

/** Search key: lower case (`title_search_key`; SQLite `LIKE` only folds ASCII). */
export function titleSearchKey(title: string): string {
  return title.toLowerCase();
}

/** `upsert_scanned_batch`: one transaction per call (~50 rows). Mark vanished paths
 *  with `markMissingPaths` first so moves relink. */
export async function upsertScannedBatch(folderId: number, rows: ScannedRow[], now: number): Promise<UpsertOutcome> {
  const statements: SqlStatement[] = [];
  for (const row of rows) {
    const params: SqlValue[] = [
      row.romPath,
      row.platformId,
      folderId,
      row.fileSize,
      row.mtime,
      row.crc32,
      row.sha1,
      row.title,
      titleSearchKey(row.title),
      row.region,
      row.datName,
      row.serial,
    ];
    statements.push({ sql: UPDATE_BY_PATH, params }, { sql: RELINK, params }, { sql: INSERT, params: [...params, now] });
    const meta = row.metadata;
    statements.push(
      meta
        ? {
            sql: WRITE_METADATA,
            params: [row.romPath, meta.developer, meta.publisher, meta.genre, meta.releaseYear, meta.players, meta.sourceVersion, now],
          }
        : { sql: DROP_METADATA, params: [row.romPath] },
    );
  }
  const results = await dbBatch(statements);
  const outcome: UpsertOutcome = { inserted: 0, updated: 0, relinked: 0 };
  for (let index = 0; index < results.length; index += 4) {
    outcome.updated += results[index].length;
    outcome.relinked += results[index + 1].length;
    outcome.inserted += results[index + 2].length;
  }
  return outcome;
}

/** `mark_missing_paths`: flags vanished files; returns rows changed. One transaction. */
export async function markMissingPaths(romPaths: string[]): Promise<number> {
  if (romPaths.length === 0) return 0;
  const results = await dbBatch(
    romPaths.map((path) => ({ sql: "UPDATE games SET missing = 1 WHERE rom_path = ? AND missing = 0 RETURNING id", params: [path] })),
  );
  return results.reduce((sum, rows) => sum + rows.length, 0);
}

/** `file_fingerprints` (+ platform, so unchanged cartridges skip classification). */
export async function fileFingerprints(folderId: number): Promise<Record<string, KnownFile>> {
  const rows = await dbQuery<{ rom_path: string; file_size: number; file_mtime_ns: number; platform_id: string }>(
    "SELECT rom_path, file_size, file_mtime_ns, platform_id FROM games WHERE folder_id = ? AND missing = 0",
    [folderId],
  );
  const known: Record<string, KnownFile> = {};
  for (const row of rows) known[row.rom_path] = { size: row.file_size, mtime: row.file_mtime_ns, platformId: row.platform_id };
  return known;
}
