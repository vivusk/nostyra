//! Why a DAT file could not be loaded.

use std::path::PathBuf;

/// DAT load failure; callers fall back to an empty index (games need review).
#[derive(Debug, thiserror::Error)]
pub enum DatLoadError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid DAT file {path}: {message}")]
    Parse { path: PathBuf, message: String },
}
