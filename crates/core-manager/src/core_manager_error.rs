//! One error type for every way a core install can fail, so the launcher can
//! show a readable message and decide whether a retry makes sense.

use std::path::PathBuf;

use http_download_util::DownloadError;

/// Why a core could not be resolved, downloaded, verified or installed.
#[derive(Debug, thiserror::Error)]
pub enum CoreManagerError {
    /// The core id is not in the buildbot catalog (only cores we ship support for).
    #[error("unknown core '{0}'")]
    UnknownCore(String),

    /// The buildbot publishes no build of this core for the running OS/arch.
    #[error("no buildbot core for {os}/{arch}")]
    UnsupportedPlatform { os: String, arch: String },

    /// Refused a non-HTTPS URL: a core is native code, so transport must be authenticated.
    #[error("refusing non-HTTPS download URL {0}")]
    InsecureUrl(String),

    /// Network or HTTP failure (DNS, TLS, non-2xx status, connection drop).
    #[error("core download failed: {0}")]
    Download(#[from] Box<ureq::Error>),

    /// Archive or extracted library is bigger than the safety cap.
    #[error("core download exceeds the {limit_bytes} byte size limit")]
    TooLarge { limit_bytes: u64 },

    /// The user cancelled through the cancel flag.
    #[error("core download cancelled")]
    Cancelled,

    /// The downloaded bytes are not a readable zip archive.
    #[error("core archive is not a valid zip: {0}")]
    InvalidArchive(#[from] zip::result::ZipError),

    /// The zip has no top-level entry with the expected library file name.
    #[error("core archive does not contain {0}")]
    LibraryNotInArchive(String),

    /// The core's system-files archive is unusable (unsafe entry, marker missing).
    #[error("core system files archive is broken: {0}")]
    AssetsArchive(String),

    /// The injected probe rejected the file (not a loadable libretro core).
    #[error("downloaded core failed verification: {0}")]
    ProbeFailed(String),

    /// Windows refuses to replace a library that a running game still has loaded.
    #[error("core {} is in use by a running game; close the game and try again", path.display())]
    CoreInUse { path: PathBuf },

    /// Download failure classified by `http-download-util` (anything not
    /// mapped onto a more specific variant above).
    #[error("core download failed: {0}")]
    Network(#[source] DownloadError),

    /// Local file system failure while writing or installing the core.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl CoreManagerError {
    /// Wraps an I/O error with the action and path that failed, for readable messages.
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

impl From<ureq::Error> for CoreManagerError {
    fn from(error: ureq::Error) -> Self {
        Self::Download(Box::new(error))
    }
}

/// Keeps the install error surface stable: callers (and the failure tests)
/// still match `Cancelled` / `TooLarge` / `Io` after the copy moved crates.
impl From<DownloadError> for CoreManagerError {
    fn from(error: DownloadError) -> Self {
        match error {
            DownloadError::Cancelled => Self::Cancelled,
            DownloadError::TooLarge { limit_bytes } => Self::TooLarge { limit_bytes },
            DownloadError::Io { context, source } => Self::Io { context, source },
            other => Self::Network(other),
        }
    }
}
