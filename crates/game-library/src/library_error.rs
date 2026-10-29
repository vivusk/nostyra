//! Error type for scanning and DAT loading.

use std::path::PathBuf;

/// Scan/DAT failure. Per-file read errors during a scan are collected in the summary
/// instead; these errors stop the operation.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid DAT file {path}: {message}")]
    DatParse { path: PathBuf, message: String },
    #[error(transparent)]
    Storage(#[from] storage_db::StorageError),
    #[error("cannot start scan worker threads: {0}")]
    ThreadPool(String),
    #[error("library folder not found: {0}")]
    FolderNotFound(PathBuf),
}
