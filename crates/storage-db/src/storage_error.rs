//! Error type for every storage-db operation.

/// Storage failure. The launcher shows these as "database error" with the source message.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("cannot create database directory {path}: {source}")]
    CreateDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A user-supplied value was rejected (message is shown as is).
    #[error("{0}")]
    InvalidInput(String),
}

/// Result alias for storage-db.
pub type StorageResult<T> = Result<T, StorageError>;
