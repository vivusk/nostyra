//! Opens the launcher database and brings its schema up to date.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use crate::storage_error::{StorageError, StorageResult};

/// Ordered, append-only schema migrations (`PRAGMA user_version` tracks progress).
const MIGRATION_LIST: &[M<'static>] = &[
    M::up(include_str!("../migrations/001_initial_schema.sql")),
    M::up(include_str!(
        "../migrations/002_metadata_artwork_favorites.sql"
    )),
    M::up(include_str!("../migrations/003_nes_platform.sql")),
    M::up(include_str!("../migrations/004_handheld_platforms.sql")),
    M::up(include_str!("../migrations/005_playstation_platform.sql")),
    M::up(include_str!("../migrations/006_sega_platforms.sql")),
    M::up(include_str!("../migrations/007_nintendo_64_platform.sql")),
    M::up(include_str!("../migrations/008_dreamcast_platform.sql")),
    M::up(include_str!("../migrations/009_psp_platform.sql")),
    M::up(include_str!("../migrations/010_nds_platform.sql")),
    M::up(include_str!("../migrations/011_gamecube_wii_platforms.sql")),
];
const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATION_LIST);

/// How long a statement waits on a locked database before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens (creating if needed) the database file at `path` and migrates it to the latest schema.
///
/// WAL lets UI reads proceed while a scan transaction writes.
pub fn open_database(path: &Path) -> StorageResult<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| StorageError::CreateDir {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // NORMAL is safe with WAL: a power cut may lose the last commit but never corrupts.
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    prepare_and_migrate(&mut conn)?;
    Ok(conn)
}

/// In-memory database with the full schema (tests, benchmarks).
pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    prepare_and_migrate(&mut conn)?;
    Ok(conn)
}

fn prepare_and_migrate(conn: &mut Connection) -> StorageResult<()> {
    // Per connection, and must be set outside a transaction.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    MIGRATIONS.to_latest(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn file_database_uses_wal_and_seeds_snes() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-db-test-")
            .tempdir()
            .unwrap();
        let path = temp.path().join("nested").join("nostyra.db");
        let conn = open_database(&path).unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        let name: String = conn
            .query_row("SELECT name FROM platforms WHERE id = 'snes'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "Super Nintendo");
        drop(conn);
        // Reopening an up-to-date database is a no-op migration.
        open_database(&path).unwrap();
    }

    #[test]
    fn existing_v2_database_migrates_to_the_nes_row() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::from_slice(&MIGRATION_LIST[..2])
            .to_latest(&mut conn)
            .unwrap();
        let count = |conn: &Connection| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM platforms WHERE id = 'nes'", [], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(count(&conn), 0);
        MIGRATIONS.to_latest(&mut conn).unwrap();
        assert_eq!(count(&conn), 1);
        let fresh = open_in_memory_database().unwrap();
        let core: String = fresh
            .query_row("SELECT core_id FROM platforms WHERE id = 'nes'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(core, "mesen");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_in_memory_database().unwrap();
        let result = conn.execute(
            "INSERT INTO play_sessions (game_id, started_at, duration_s) VALUES (999, 0, 1)",
            [],
        );
        assert!(result.is_err());
    }
}
