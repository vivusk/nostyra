//! The account-wide `index.json`: unit key → last committed generation, so the
//! launcher's startup pass finds changed units with one GET. Written after every
//! commit, conditionally; on a lost race it is re-read and merged per key (each PC
//! only ever sets its own unit's entry). A non-empty `repair` list (a blob missing /
//! corrupt in the visible cloud folder) marks the unit changed for every PC.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::{read_optional, CloudStorageProvider, Rev};

/// Remote path of the index (under the provider root).
pub const INDEX_PATH: &str = "index.json";
/// Index format this build reads and writes.
pub const INDEX_FORMAT: u32 = 1;
/// Conditional index writes tried this often before giving up (logged).
const INDEX_WRITE_TRIES: usize = 3;

/// A manifest entry whose blob was found missing / corrupt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairRef {
    pub path: String,
    /// The broken blob path (under the unit folder).
    pub blob: String,
}

/// One unit in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub generation: u64,
    /// Unix seconds of the commit.
    pub updated_at: u64,
    pub device_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repair: Vec<RepairRef>,
}

/// `index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudIndex {
    pub format: u32,
    #[serde(default)]
    pub units: BTreeMap<String, IndexEntry>,
}

impl Default for CloudIndex {
    fn default() -> Self {
        Self {
            format: INDEX_FORMAT,
            units: BTreeMap::new(),
        }
    }
}

/// Reads the index (absent = empty, `None` rev). Another format is
/// `UnsupportedRemoteFormat`: never overwritten.
pub fn read_index(
    provider: &dyn CloudStorageProvider,
) -> Result<(CloudIndex, Option<Rev>), CloudError> {
    let Some((bytes, rev)) = read_optional(provider, INDEX_PATH)? else {
        return Ok((CloudIndex::default(), None));
    };
    let index =
        parse_index(&bytes)?.ok_or_else(|| CloudError::Other("invalid index.json".to_string()))?;
    Ok((index, Some(rev)))
}

/// Like [`read_index`], but an `index.json` that does not parse (mangled by hand /
/// a desktop client) comes back as an empty index with its rev, so the next write
/// replaces it instead of leaving it stale for good. Another format still fails.
pub(crate) fn read_index_or_reset(
    provider: &dyn CloudStorageProvider,
) -> Result<(CloudIndex, Option<Rev>), CloudError> {
    let Some((bytes, rev)) = read_optional(provider, INDEX_PATH)? else {
        return Ok((CloudIndex::default(), None));
    };
    let index = parse_index(&bytes)?.unwrap_or_else(|| {
        tracing::warn!(target: "cloud_sync", "index.json does not parse: rebuilding it");
        CloudIndex::default()
    });
    Ok((index, Some(rev)))
}

/// `Ok(None)` = not a valid index; any other `format` (checked first, so a newer
/// layout is never taken for garbage) = `UnsupportedRemoteFormat`.
fn parse_index(bytes: &[u8]) -> Result<Option<CloudIndex>, CloudError> {
    let format = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("format").and_then(serde_json::Value::as_u64));
    if format.is_some_and(|format| format != u64::from(INDEX_FORMAT)) {
        return Err(CloudError::UnsupportedRemoteFormat);
    }
    Ok(serde_json::from_slice::<CloudIndex>(bytes).ok())
}

/// Applies `change` to the index and writes it conditionally; on `RevMismatch`
/// re-reads and re-applies (a per-key merge: `change` only touches its unit), up to
/// [`INDEX_WRITE_TRIES`] writes. `known` = an index + rev already read.
pub fn update_index(
    provider: &dyn CloudStorageProvider,
    known: Option<(CloudIndex, Option<Rev>)>,
    change: impl Fn(&mut CloudIndex),
) -> Result<(), CloudError> {
    let mut current = match known {
        Some(known) => known,
        None => read_index(provider)?,
    };
    for _ in 0..INDEX_WRITE_TRIES {
        let (mut index, rev) = current;
        change(&mut index);
        let bytes = serde_json::to_vec_pretty(&index)
            .map_err(|err| CloudError::Other(format!("encode index.json: {err}")))?;
        match provider.write(INDEX_PATH, &bytes, rev.as_ref()) {
            Ok(_) => return Ok(()),
            Err(CloudError::RevMismatch) => current = read_index(provider)?,
            Err(err) => return Err(err),
        }
    }
    Err(CloudError::RevMismatch)
}

/// Units the startup pass must sync: in the index with a generation other than this
/// PC's base generation (or no base), or flagged for repair. `bases` = unit key →
/// base generation of this account.
pub fn changed_units(index: &CloudIndex, bases: &BTreeMap<String, u64>) -> Vec<String> {
    index
        .units
        .iter()
        .filter(|(key, entry)| {
            !entry.repair.is_empty() || bases.get(key.as_str()) != Some(&entry.generation)
        })
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(test)]
#[path = "cloud_index_tests.rs"]
mod tests;
