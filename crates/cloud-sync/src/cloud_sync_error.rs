//! Errors of the local sync core (snapshot, manifests, base store). Provider /
//! network errors are a separate type owned by the provider trait.

use std::path::PathBuf;

/// What went wrong reading local saves or decoding a manifest.
#[derive(Debug, thiserror::Error)]
pub enum CloudSyncError {
    /// A local file or folder could not be read / written.
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A manifest written by a newer Nostyra (or not a manifest at all). The caller
    /// must never overwrite it: an older build would drop fields it cannot see.
    #[error("unsupported cloud manifest format {found:?} (this build reads format {expected})")]
    UnsupportedRemoteFormat { found: Option<u64>, expected: u32 },
    /// Malformed JSON in a manifest.
    #[error("invalid {what} JSON: {source}")]
    Json {
        what: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

impl CloudSyncError {
    /// Io error naming `path`.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
