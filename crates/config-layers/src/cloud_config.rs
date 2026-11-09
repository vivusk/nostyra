//! `[cloud]` section (M19 cloud saves). **Global layer only**, like `[achievements]`:
//! one cloud account per PC, so the table is stripped from the system, core and game
//! layers before merging. The launcher reads it; the runner ignores it.

use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::known_key_checks::KeyCheck;

/// Section name of the cloud-save settings.
pub const CLOUD_SECTION: &str = "cloud";

/// Which storage the saves sync with (one at a time).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProviderId {
    /// Cloud saves off.
    #[default]
    Off,
    GoogleDrive,
    Onedrive,
    Dropbox,
    /// A folder a Drive / OneDrive / Dropbox desktop client already syncs.
    LocalFolder,
}

impl CloudProviderId {
    /// Every value as written in `global.toml`.
    pub const ALL: [(Self, &'static str); 5] = [
        (Self::Off, "off"),
        (Self::GoogleDrive, "google_drive"),
        (Self::Onedrive, "onedrive"),
        (Self::Dropbox, "dropbox"),
        (Self::LocalFolder, "local_folder"),
    ];

    /// The TOML / base-folder id (`"local_folder"`, …).
    pub fn as_str(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(id, _)| *id == self)
            .map_or("off", |(_, name)| name)
    }

    /// Parses a TOML value; `None` for an unknown id.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, name)| *name == value)
            .map(|(id, _)| *id)
    }
}

/// Cloud-save settings (launcher Settings → Cloud saves, global layer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudConfig {
    pub provider: CloudProviderId,
    /// Folder of the Local folder provider (absolute path; empty = not chosen).
    pub local_folder: String,
    /// "Also sync save states" (`slotN.*`, `auto.*`).
    pub sync_states: bool,
    /// Background sync when the launcher starts.
    pub sync_on_start: bool,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            provider: CloudProviderId::Off,
            local_folder: String::new(),
            sync_states: true,
            sync_on_start: true,
        }
    }
}

/// `[cloud]` keys only the dedicated cloud commands write (they validate the
/// folder and handle the account): never through a generic config-value write.
pub const CLOUD_COMMAND_ONLY_KEYS: [&str; 2] = ["cloud.provider", "cloud.local_folder"];

/// Value checks of the `[cloud]` keys.
pub(crate) const CLOUD_KEY_CHECKS: [KeyCheck; 4] = [
    ("cloud", "provider", |v: &Value| {
        v.as_str().and_then(CloudProviderId::parse).is_some()
    }),
    ("cloud", "local_folder", |v: &Value| {
        v.as_str()
            .is_some_and(|path| path.is_empty() || Path::new(path).is_absolute())
    }),
    ("cloud", "sync_states", |v: &Value| v.is_bool()),
    ("cloud", "sync_on_start", |v: &Value| v.is_bool()),
];

/// Removes `[cloud]` from a non-global layer (logged: the value has no effect).
pub(crate) fn strip_cloud_section(layer: &mut Table, source: &Path) {
    if layer.remove(CLOUD_SECTION).is_some() {
        tracing::warn!(layer = %source.display(),
            "[cloud] is a global setting; ignored in this layer");
    }
}

#[cfg(test)]
#[cfg(feature = "fs")]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;
    use crate::{resolve_config, ConfigLayerPaths};

    #[test]
    fn provider_ids_round_trip() {
        for (id, name) in CloudProviderId::ALL {
            assert_eq!(id.as_str(), name);
            assert_eq!(CloudProviderId::parse(name), Some(id));
        }
        assert_eq!(CloudProviderId::parse("icloud"), None);
    }

    #[test]
    fn cloud_comes_from_the_global_layer_only() {
        let temp = tempfile::tempdir().unwrap();
        let layers = ConfigLayerPaths::new(temp.path(), "sys", "core", Path::new("Game.rom"), "");
        let write = |path: &Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        assert_eq!(resolve_config(&layers, None).cloud, CloudConfig::default());
        let folder = temp
            .path()
            .join("Drive")
            .to_string_lossy()
            .replace('\\', "/");
        write(
            &layers.global,
            &format!(
                "[cloud]\nprovider = \"local_folder\"\nlocal_folder = \"{folder}\"\n\
                 sync_states = false\nsync_on_start = 3\n"
            ),
        );
        write(
            &layers.game,
            "[cloud]\nprovider = \"off\"\nsync_states = true\n",
        );
        let config = resolve_config(&layers, None).cloud;
        assert_eq!(config.provider, CloudProviderId::LocalFolder);
        assert_eq!(config.local_folder, folder);
        assert!(
            !config.sync_states,
            "global value applies, game layer ignored"
        );
        assert!(config.sync_on_start, "invalid value → default");
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let layers = ConfigLayerPaths::new(temp.path(), "sys", "core", Path::new("Game.rom"), "");
        std::fs::create_dir_all(layers.global.parent().unwrap()).unwrap();
        std::fs::write(
            &layers.global,
            "[cloud]\nprovider = \"icloud\"\nlocal_folder = \"relative/dir\"\n",
        )
        .unwrap();
        assert_eq!(resolve_config(&layers, None).cloud, CloudConfig::default());
    }
}
