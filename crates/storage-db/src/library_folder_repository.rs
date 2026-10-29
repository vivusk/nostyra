//! ROM folders chosen by the user.

use rusqlite::{params, Connection};

use crate::domain_types::LibraryFolder;
use crate::storage_error::StorageResult;

/// Adds a folder; adding an existing path returns the existing row (idempotent).
pub fn add_library_folder(conn: &Connection, path: &str, now: i64) -> StorageResult<LibraryFolder> {
    conn.execute(
        "INSERT OR IGNORE INTO library_folders (path, added_at) VALUES (?1, ?2)",
        params![path, now],
    )?;
    Ok(conn.query_row(
        "SELECT id, path, added_at FROM library_folders WHERE path = ?1",
        params![path],
        |r| {
            Ok(LibraryFolder {
                id: r.get(0)?,
                path: r.get(1)?,
                added_at: r.get(2)?,
            })
        },
    )?)
}

/// Removes a folder. Its games are detached and flagged missing (hidden), never deleted,
/// so play history comes back if the folder is added again. Returns false if unknown.
pub fn remove_library_folder(conn: &mut Connection, folder_id: i64) -> StorageResult<bool> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE games SET missing = 1, folder_id = NULL WHERE folder_id = ?1",
        params![folder_id],
    )?;
    let removed = tx.execute(
        "DELETE FROM library_folders WHERE id = ?1",
        params![folder_id],
    )?;
    tx.commit()?;
    Ok(removed > 0)
}

/// All folders, oldest first.
pub fn list_library_folders(conn: &Connection) -> StorageResult<Vec<LibraryFolder>> {
    let mut stmt =
        conn.prepare_cached("SELECT id, path, added_at FROM library_folders ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(LibraryFolder {
            id: r.get(0)?,
            path: r.get(1)?,
            added_at: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
