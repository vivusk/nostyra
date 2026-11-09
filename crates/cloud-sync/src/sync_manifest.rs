//! The remote `manifest.json` of a unit: the commit point of the sync model.
//!
//! Commit model: a push uploads every changed file as a new **immutable blob**
//! `blobs/<gen>-<device8>/<path>` (gen = the generation being committed, device8 =
//! first 8 hex digits of the pushing device id), then writes the manifest with
//! `generation + 1` last. Unchanged files keep pointing at older blob dirs. Until the
//! manifest is written nothing is visible to other PCs, so a crash or a concurrent
//! push can never corrupt the committed version.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cloud_sync_error::CloudSyncError;

/// Manifest format this build reads and writes.
pub const MANIFEST_FORMAT: u32 = 1;
/// Tombstones (deleted files) are kept this long, then dropped on write.
pub const TOMBSTONE_MAX_AGE_SECS: u64 = 90 * 24 * 3600;

/// One file of a unit as committed remotely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Lower-case hex sha256 of the content (provider-independent).
    pub sha256: String,
    pub size: u64,
    /// File mtime on the pushing PC, unix seconds (informative only).
    pub modified_at: u64,
    /// Blob path under the unit folder: `blobs/<gen>-<device8>/<path>`.
    pub blob: String,
    /// Unix seconds of the deletion: a tombstone, the file is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<u64>,
}

impl FileEntry {
    /// The content sha when the file exists (not a tombstone).
    pub fn live_sha(&self) -> Option<&str> {
        self.deleted_at.is_none().then_some(self.sha256.as_str())
    }
}

/// `games/<sys>/<sha1>/manifest.json` or `shared/<key>/manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteManifest {
    pub format: u32,
    /// Bumped by one on every commit.
    pub generation: u64,
    /// Unix seconds of the commit.
    pub updated_at: u64,
    /// Device that wrote this generation.
    pub device_id: String,
    pub device_name: String,
    /// Relative path (`/`-separated) → entry, tombstones included.
    pub files: BTreeMap<String, FileEntry>,
}

/// Probe for the format before the full decode (a newer format may change any field).
#[derive(Deserialize)]
struct FormatProbe {
    format: Option<u64>,
}

impl RemoteManifest {
    /// An empty generation-0 manifest (nothing committed yet).
    pub fn empty(device_id: &str, device_name: &str) -> Self {
        Self {
            format: MANIFEST_FORMAT,
            generation: 0,
            updated_at: 0,
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            files: BTreeMap::new(),
        }
    }

    /// Decodes a manifest; a format other than [`MANIFEST_FORMAT`] is
    /// `UnsupportedRemoteFormat` (callers then never overwrite it).
    pub fn parse(bytes: &[u8]) -> Result<Self, CloudSyncError> {
        let json = |source| CloudSyncError::Json {
            what: "manifest",
            source,
        };
        let probe: FormatProbe = serde_json::from_slice(bytes).map_err(json)?;
        if probe.format != Some(u64::from(MANIFEST_FORMAT)) {
            return Err(CloudSyncError::UnsupportedRemoteFormat {
                found: probe.format,
                expected: MANIFEST_FORMAT,
            });
        }
        serde_json::from_slice(bytes).map_err(json)
    }

    /// Encodes for upload after dropping tombstones older than
    /// [`TOMBSTONE_MAX_AGE_SECS`] at `now` (unix seconds).
    pub fn encode(&mut self, now: u64) -> Result<Vec<u8>, CloudSyncError> {
        self.drop_expired_tombstones(now);
        serde_json::to_vec_pretty(self).map_err(|source| CloudSyncError::Json {
            what: "manifest",
            source,
        })
    }

    /// Removes tombstones deleted more than [`TOMBSTONE_MAX_AGE_SECS`] before `now`.
    pub fn drop_expired_tombstones(&mut self, now: u64) {
        self.files.retain(|_, entry| {
            entry
                .deleted_at
                .is_none_or(|at| now.saturating_sub(at) <= TOMBSTONE_MAX_AGE_SECS)
        });
    }

    /// Whether `other` has the same writer and blob refs (tombstones included),
    /// generation aside. Different refs at an equal or lower generation than the
    /// base mean another PC won a last-writer-wins race ("not my commit").
    pub fn same_refs(&self, other: &Self) -> bool {
        self.device_id == other.device_id
            && self.files.len() == other.files.len()
            && self
                .files
                .iter()
                .zip(&other.files)
                .all(|((pa, a), (pb, b))| {
                    pa == pb && a.blob == b.blob && a.deleted_at.is_some() == b.deleted_at.is_some()
                })
    }
}

/// `blobs/<generation>-<device8>/<path>`: where a push targeting `generation`
/// uploads `path`.
pub fn blob_path(generation: u64, device_id: &str, path: &str) -> String {
    let device8: String = device_id.chars().take(8).collect();
    format!("blobs/{generation}-{device8}/{path}")
}

#[cfg(test)]
#[path = "sync_manifest_tests.rs"]
mod tests;
