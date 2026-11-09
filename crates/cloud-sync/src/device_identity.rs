//! This PC's identity in the cloud: `<data>/cloud/device.json` with a random
//! 128-bit hex id and the computer name. The id names this PC's blob dirs and the
//! manifest writer. A data folder copied to another PC (stored name ≠ current
//! `COMPUTERNAME`) gets a fresh id, so two PCs never share one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cloud_sync_error::CloudSyncError;

/// Fallback when the OS gives no computer name.
const UNKNOWN_DEVICE_NAME: &str = "PC";

/// Who writes a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    /// 32 lower-case hex digits.
    pub device_id: String,
    /// Shown in the conflict dialog ("Cloud (<device>, <time>)").
    pub device_name: String,
}

impl DeviceIdentity {
    /// Loads `<data>/cloud/device.json`, creating it (or a new id when the stored
    /// name differs from this computer's name).
    pub fn load_or_create(data_root: &Path) -> Result<Self, CloudSyncError> {
        Self::load_or_create_named(data_root, &current_computer_name())
    }

    /// [`Self::load_or_create`] with an explicit computer name (tests, two-PC sims).
    pub fn load_or_create_named(data_root: &Path, name: &str) -> Result<Self, CloudSyncError> {
        let path = device_file(data_root);
        if let Some(stored) = read_stored(&path) {
            if stored.device_name == name && is_hex_id(&stored.device_id) {
                return Ok(stored);
            }
            tracing::info!(target: "cloud_sync", "computer name changed: new device id");
        }
        let identity = Self {
            device_id: random_hex_id()?,
            device_name: name.to_string(),
        };
        let bytes =
            serde_json::to_vec_pretty(&identity).map_err(|source| CloudSyncError::Json {
                what: "device",
                source,
            })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| CloudSyncError::io(parent, err))?;
        }
        config_layers::write_atomically(&path, &bytes)
            .map_err(|err| CloudSyncError::io(&path, err))?;
        Ok(identity)
    }

    /// First 8 hex digits of the id (blob dir suffix).
    pub fn device8(&self) -> &str {
        self.device_id.get(..8).unwrap_or(&self.device_id)
    }
}

/// `<data>/cloud/device.json`.
pub fn device_file(data_root: &Path) -> PathBuf {
    data_root.join("cloud").join("device.json")
}

/// `%COMPUTERNAME%` (Windows) / `$HOSTNAME`, else "PC".
pub fn current_computer_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|name| name.trim().to_string())
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| UNKNOWN_DEVICE_NAME.to_string())
}

fn read_stored(path: &Path) -> Option<DeviceIdentity> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn is_hex_id(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn random_hex_id() -> Result<String, CloudSyncError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|err| CloudSyncError::io("<os random>", std::io::Error::other(err.to_string())))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable_and_regenerated_when_the_computer_name_differs() {
        let dir = tempfile::tempdir().unwrap();
        let first = DeviceIdentity::load_or_create_named(dir.path(), "PC-A").unwrap();
        assert!(is_hex_id(&first.device_id));
        assert_eq!(first.device8().len(), 8);
        let again = DeviceIdentity::load_or_create_named(dir.path(), "PC-A").unwrap();
        assert_eq!(again, first);
        let copied = DeviceIdentity::load_or_create_named(dir.path(), "PC-B").unwrap();
        assert_ne!(copied.device_id, first.device_id);
        assert_eq!(copied.device_name, "PC-B");
        let stored = read_stored(&device_file(dir.path())).unwrap();
        assert_eq!(stored, copied);
    }
}
