//! Local (non-network) failures: an image that does not decode or is too
//! big, an index that does not parse, a cache write that fails. Network
//! failures stay `http_download_util::DownloadError`.

/// Why an artwork step failed after (or without) the download.
#[derive(Debug, thiserror::Error)]
pub enum ArtworkError {
    /// The bytes are not an acceptable image (corrupt, wrong format, or
    /// dimensions over the decode cap).
    #[error("invalid image: {0}")]
    InvalidImage(String),

    /// The GitHub tree JSON could not be parsed.
    #[error("invalid thumbnail index: {0}")]
    InvalidIndex(String),

    /// Local file system failure in the artwork cache.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl ArtworkError {
    /// Wraps an I/O error with the action and path that failed.
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
