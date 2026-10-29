//! Error type for core loading and session operations.

use std::path::PathBuf;

/// Everything that can go wrong hosting a core.
///
/// Split into core-side vs game-side failures so the runner can map them to
/// distinct exit codes (bad core = 3, bad ROM = 4) via [`HostError::is_core_error`].
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("failed to load core library {path}: {source}")]
    LoadLibrary {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("core is missing required symbol `{0}` (not a libretro core?)")]
    MissingSymbol(&'static str),
    #[error("core reports libretro API version {0}, expected 1")]
    ApiVersion(u32),
    #[error("a libretro core session is already active in this process")]
    SessionAlreadyActive,
    #[error("path contains an interior NUL byte: {0}")]
    InvalidPath(PathBuf),
    #[error("failed to read ROM {path}: {source}")]
    ReadRom {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot stage the content for the core at {path}: {source}")]
    StageContent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// `retro_load_game` returned false, with the core's reason when it gave one (its
    /// last on-screen message, else its last error log line).
    #[error("core rejected the game (retro_load_game returned false){}", .0.as_deref().map(|m| format!(": {m}")).unwrap_or_default())]
    LoadGameRejected(Option<String>),
    #[error("core does not support save states")]
    SerializeUnsupported,
    #[error("core is not ready to create a save state yet")]
    SerializeNotReady,
    #[error("core failed to create a save state")]
    SerializeFailed,
    #[error("core rejected the save state")]
    UnserializeFailed,
    /// The core's states need its hardware context and the session has none (Dolphin
    /// on its Null renderer crashes in `retro_serialize_size`): no state call is made.
    #[error("save states are unavailable without hardware rendering")]
    StatesNeedHwContext,
    #[error("a host callback panicked; core state may be inconsistent")]
    CallbackPanicked,
    /// The core asked to stop (`RETRO_ENVIRONMENT_SHUTDOWN`), with its last message.
    #[error("the core stopped after an emulation error{}", .0.as_deref().map(|m| format!(": {m}")).unwrap_or_default())]
    CoreShutdown(Option<String>),
    #[error("core refused to switch to disc {0}")]
    DiscSwapRejected(u32),
    /// The core needs a graphics API (e.g. OpenGL 3.3) this host cannot provide.
    #[error("graphics: {0}")]
    HwRenderUnavailable(String),
}

impl HostError {
    /// True when the core binary itself is unusable (vs. the game/ROM or a runtime fault).
    pub fn is_core_error(&self) -> bool {
        matches!(
            self,
            Self::LoadLibrary { .. } | Self::MissingSymbol(_) | Self::ApiVersion(_)
        )
    }
}

/// Convenience alias used across the crate.
pub type HostResult<T> = Result<T, HostError>;
