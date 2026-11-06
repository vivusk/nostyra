// ROM folders in the web library DB, mirroring
// crates/storage-db/src/library_folder_repository.rs. On the web `path` is
// `web:<token>`; the folder's handle lives in IndexedDB under the token.
import { dbBatch, dbQuery } from "./db-client";

export interface LibraryFolderRow {
  id: number;
  path: string;
  addedAt: number;
}

/** `list_library_folders`: all folders, oldest first. */
export async function listLibraryFolderRows(): Promise<LibraryFolderRow[]> {
  const rows = await dbQuery<{ id: number; path: string; added_at: number }>(
    "SELECT id, path, added_at FROM library_folders ORDER BY id",
  );
  return rows.map((row) => ({ id: row.id, path: row.path, addedAt: row.added_at }));
}

/** `add_library_folder`: idempotent by path; returns the row. */
export async function addLibraryFolderRow(path: string, now: number): Promise<LibraryFolderRow> {
  const [, rows] = await dbBatch([
    { sql: "INSERT OR IGNORE INTO library_folders (path, added_at) VALUES (?, ?)", params: [path, now] },
    { sql: "SELECT id, path, added_at FROM library_folders WHERE path = ?", params: [path] },
  ]);
  const row = rows[0];
  return { id: Number(row.id), path: String(row.path), addedAt: Number(row.added_at) };
}

/** `remove_library_folder`: its games are detached and flagged missing (hidden), never
 *  deleted, so play history comes back through the sha1 relink if it is added again.
 *  Returns false if unknown. */
export async function removeLibraryFolderRow(folderId: number): Promise<boolean> {
  const [, removed] = await dbBatch([
    { sql: "UPDATE games SET missing = 1, folder_id = NULL WHERE folder_id = ?", params: [folderId] },
    { sql: "DELETE FROM library_folders WHERE id = ? RETURNING id", params: [folderId] },
  ]);
  return removed.length > 0;
}
