//! Error shape returned by every Tauri command: `{ kind, message }`, so the UI can
//! show a readable message and branch on the category.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// "storage" | "scan" | "core" | "core_missing" | "session" | "artwork" | "busy" |
    /// "invalid".
    pub kind: &'static str,
    pub message: String,
    /// `core_missing` only: the registry core to install.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_id: Option<&'static str>,
}

impl CommandError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            core_id: None,
        }
    }

    /// Play refused: the core for `platform_id` games is not installed.
    pub fn core_missing(core: &'static system_catalog::CoreSpec, platform_id: &str) -> Self {
        let system = system_catalog::system(platform_id).map_or(platform_id, |s| s.short_label);
        Self {
            kind: "core_missing",
            message: format!(
                "{system} games need the {} core. Install it from Settings.",
                core.display_name
            ),
            core_id: Some(core.id),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }
}

impl From<storage_db::StorageError> for CommandError {
    fn from(err: storage_db::StorageError) -> Self {
        match err {
            storage_db::StorageError::InvalidInput(_) => Self::invalid(err.to_string()),
            _ => Self::new("storage", err.to_string()),
        }
    }
}

impl From<game_library::LibraryError> for CommandError {
    fn from(err: game_library::LibraryError) -> Self {
        Self::new("scan", err.to_string())
    }
}

impl From<core_manager::CoreManagerError> for CommandError {
    fn from(err: core_manager::CoreManagerError) -> Self {
        Self::new("core", err.to_string())
    }
}

impl From<artwork_cache::ArtworkError> for CommandError {
    fn from(err: artwork_cache::ArtworkError) -> Self {
        Self::new("artwork", err.to_string())
    }
}

impl From<config_layers::ConfigError> for CommandError {
    fn from(err: config_layers::ConfigError) -> Self {
        use config_layers::ConfigError;
        match err {
            ConfigError::InvalidKey(_) | ConfigError::InvalidValue { .. } => {
                Self::invalid(err.to_string())
            }
            // Unreadable / non-TOML layer file or a failed write.
            ConfigError::Io { .. } | ConfigError::Parse { .. } | ConfigError::Serialize(_) => {
                Self::new("storage", err.to_string())
            }
        }
    }
}

/// Result alias for commands.
pub type CommandResult<T> = Result<T, CommandError>;
