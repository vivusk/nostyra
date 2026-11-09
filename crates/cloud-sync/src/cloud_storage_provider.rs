//! The storage a sync account offers: path-based operations on one remote root
//! (`nostyra-cloud/v1/` inside the app folder). Paths are relative, `/`-separated,
//! never `..`. Providers stay ~5 calls each; all sync rules live in the executor.

use std::io::{Read, Write};
use std::sync::Arc;

use crate::cloud_error::CloudError;
use crate::http_transport::RequestBudget;

/// Opaque provider revision of a file (etag / rev / our own sha), used only for
/// conditional writes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rev(pub String);

/// One child of a listed remote folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    /// Plain name (no `/`).
    pub name: String,
    pub is_dir: bool,
}

/// One sync account's remote storage.
pub trait CloudStorageProvider: Send + Sync {
    /// Children of `dir` (`""` = root). A missing folder is `NotFound`.
    fn list(&self, dir: &str) -> Result<Vec<ListEntry>, CloudError>;

    /// The whole file and its revision (small files: manifests, `index.json`). A
    /// file over [`crate::metadata_read_cap::MAX_METADATA_BYTES`] is `Other`
    /// (treated as corrupt, never buffered whole).
    fn read(&self, path: &str) -> Result<(Vec<u8>, Rev), CloudError>;

    /// Streams the file into `out` chunk by chunk (blobs: never held in memory) and
    /// returns its revision. A writer error (e.g. the sync was cancelled) aborts the
    /// download.
    fn read_to(&self, path: &str, out: &mut dyn Write) -> Result<Rev, CloudError>;

    /// Creates / replaces `path`. `expect = Some(rev)`: only when the file still has
    /// that revision (else `RevMismatch`; where the API has no conditional writes the
    /// provider re-checks the revision just before writing). `None`: no precondition.
    fn write(&self, path: &str, bytes: &[u8], expect: Option<&Rev>) -> Result<Rev, CloudError>;

    /// Streams `len` bytes from `reader` into `path` (chunked upload for big files;
    /// no precondition: blobs are written once at a fresh path). A reader error
    /// (e.g. the sync was cancelled) aborts the upload.
    fn write_large(&self, path: &str, reader: &mut dyn Read, len: u64) -> Result<Rev, CloudError>;

    /// Deletes one file; already gone = ok.
    fn delete(&self, path: &str) -> Result<(), CloudError>;

    /// Deletes a folder and everything below it; already gone = ok.
    fn delete_dir(&self, dir: &str) -> Result<(), CloudError>;

    /// Human-readable account name for Settings ("user@example.com", folder path).
    fn account_label(&self) -> String;

    /// The same account with every request (token refresh included) held to
    /// `budget`, e.g. the pre-play deadline: a view sharing sign-in and caches.
    /// `None` = no network calls (local providers): use `self` as is.
    fn budgeted(&self, _budget: RequestBudget) -> Option<Arc<dyn CloudStorageProvider>> {
        None
    }
}

/// Reads `path`, mapping `NotFound` to `None`.
pub fn read_optional(
    provider: &dyn CloudStorageProvider,
    path: &str,
) -> Result<Option<(Vec<u8>, Rev)>, CloudError> {
    match provider.read(path) {
        Ok(found) => Ok(Some(found)),
        Err(CloudError::NotFound) => Ok(None),
        Err(err) => Err(err),
    }
}
