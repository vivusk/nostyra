//! Garbage collection of a unit's blob dirs after a successful commit: a
//! `blobs/<gen>-<device8>/` dir is deleted (recursively, one call) only when its
//! gen is below the new generation and neither the new nor the previous manifest
//! references any blob in it. Partially referenced dirs stay whole; a same-gen dir
//! of an in-flight push elsewhere is never touched; names that are not ours (client
//! conflict copies, user folders) are ignored. Errors are logged, never fatal.

use std::collections::BTreeSet;

use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::CloudStorageProvider;
use crate::sync_manifest::RemoteManifest;

/// Deletes unreferenced blob dirs under `<prefix>blobs/`; returns how many.
pub(crate) fn collect_garbage(
    provider: &dyn CloudStorageProvider,
    prefix: &str,
    new: &RemoteManifest,
    previous: Option<&RemoteManifest>,
) -> usize {
    let blobs = format!("{prefix}blobs");
    let entries = match provider.list(&blobs) {
        Ok(entries) => entries,
        Err(CloudError::NotFound) => return 0,
        Err(err) => {
            tracing::warn!(target: "cloud_sync", "blob GC: cannot list {blobs}: {err}");
            return 0;
        }
    };
    let referenced: BTreeSet<&str> = std::iter::once(new)
        .chain(previous)
        .flat_map(|manifest| manifest.files.values())
        .filter(|entry| entry.deleted_at.is_none())
        .filter_map(|entry| blob_dir_of(&entry.blob))
        .collect();
    let mut removed = 0;
    for entry in entries.into_iter().filter(|entry| entry.is_dir) {
        let Some(generation) = parse_blob_dir(&entry.name) else {
            continue;
        };
        if generation >= new.generation || referenced.contains(entry.name.as_str()) {
            continue;
        }
        match provider.delete_dir(&format!("{blobs}/{}", entry.name)) {
            Ok(()) => removed += 1,
            Err(err) => {
                tracing::warn!(target: "cloud_sync", "blob GC: cannot delete {blobs}/{}: {err}", entry.name);
            }
        }
    }
    removed
}

/// `blobs/<dir>/…` → `<dir>`.
fn blob_dir_of(blob: &str) -> Option<&str> {
    let rest = blob.strip_prefix("blobs/")?;
    rest.split_once('/').map(|(dir, _)| dir)
}

/// `<gen>-<8 hex>` → gen.
fn parse_blob_dir(name: &str) -> Option<u64> {
    let (generation, device8) = name.split_once('-')?;
    let hex = device8.len() == 8 && device8.chars().all(|c| c.is_ascii_hexdigit());
    hex.then(|| generation.parse().ok())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake_cloud_provider::FakeCloudProvider;
    use crate::sync_manifest::FileEntry;

    fn manifest(generation: u64, blobs: &[&str]) -> RemoteManifest {
        let mut manifest = RemoteManifest::empty("dev", "PC");
        manifest.generation = generation;
        for (i, blob) in blobs.iter().enumerate() {
            let entry = FileEntry {
                sha256: "s".into(),
                size: 1,
                modified_at: 0,
                blob: (*blob).into(),
                deleted_at: None,
            };
            manifest.files.insert(format!("f{i}"), entry);
        }
        manifest
    }

    #[test]
    fn keeps_referenced_previous_and_in_flight_dirs() {
        let fake = FakeCloudProvider::new();
        for dir in [
            "1-aaaaaaaa",
            "2-aaaaaaaa",
            "3-bbbbbbbb",
            "4-aaaaaaaa",
            "4-cccccccc",
        ] {
            fake.put(&format!("u/blobs/{dir}/f"), b"x");
        }
        fake.put("u/blobs/2-aaaaaaaa-PC (conflicted copy)/f", b"x");
        fake.put("u/blobs/notes/f", b"x");
        let previous = manifest(3, &["blobs/1-aaaaaaaa/f", "blobs/3-bbbbbbbb/f"]);
        let new = manifest(4, &["blobs/1-aaaaaaaa/f", "blobs/4-aaaaaaaa/f"]);
        assert_eq!(collect_garbage(&fake, "u/", &new, Some(&previous)), 1);
        let paths = fake.paths();
        assert!(!paths.iter().any(|p| p.starts_with("u/blobs/2-aaaaaaaa/")));
        for kept in [
            "1-aaaaaaaa",
            "3-bbbbbbbb",
            "4-aaaaaaaa",
            "4-cccccccc",
            "notes",
        ] {
            assert!(
                paths
                    .iter()
                    .any(|p| p.starts_with(&format!("u/blobs/{kept}/"))),
                "{kept}"
            );
        }
        assert_eq!(collect_garbage(&fake, "missing/", &new, None), 0);
    }
}
