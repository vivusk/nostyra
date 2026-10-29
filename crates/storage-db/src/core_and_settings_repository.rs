//! Installed cores and key/value settings: small tables, one module.

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain_types::InstalledCoreRecord;
use crate::storage_error::StorageResult;

/// Inserts or replaces the record of a downloaded core.
pub fn save_installed_core(conn: &Connection, core: &InstalledCoreRecord) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO cores (id, platform_id, library_path, version, sha256, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET platform_id = excluded.platform_id,
            library_path = excluded.library_path, version = excluded.version,
            sha256 = excluded.sha256, installed_at = excluded.installed_at",
        params![
            core.id,
            core.platform_id,
            core.library_path,
            core.version,
            core.sha256,
            core.installed_at
        ],
    )?;
    Ok(())
}

/// The installed core with this id, if any.
pub fn get_installed_core(
    conn: &Connection,
    core_id: &str,
) -> StorageResult<Option<InstalledCoreRecord>> {
    Ok(conn
        .query_row(
            "SELECT id, platform_id, library_path, version, sha256, installed_at
             FROM cores WHERE id = ?1",
            params![core_id],
            |r| {
                Ok(InstalledCoreRecord {
                    id: r.get(0)?,
                    platform_id: r.get(1)?,
                    library_path: r.get(2)?,
                    version: r.get(3)?,
                    sha256: r.get(4)?,
                    installed_at: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// A setting value, if set.
pub fn get_setting(conn: &Connection, key: &str) -> StorageResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

/// Sets (inserts or overwrites) a setting.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
