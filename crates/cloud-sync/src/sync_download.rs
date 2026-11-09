//! Downloading one blob straight into a local file while hashing the bytes as they
//! stream (never held in memory: state blobs reach 256 MiB). Every chunk checks the
//! cancel flag, so a pre-play budget can stop a big download mid-file. Used for
//! staged pulls and "Keep this PC" cloud backups.

use std::io::{self, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::cloud_error::CloudError;
use crate::content_hash::to_hex;
use crate::sync_outcome::{SyncContext, SyncUnitError};
use crate::sync_progress::SyncProgress;

/// Streams `remote_path` into `dest` (parents created). `Some(len)` when the bytes
/// hash to `expected_sha`; `None` when the blob is missing or does not match (then
/// `dest` is removed).
pub(crate) fn download_verified(
    ctx: &SyncContext<'_>,
    remote_path: &str,
    dest: &Path,
    expected_sha: &str,
) -> Result<Option<u64>, SyncUnitError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| SyncUnitError::io(parent, err))?;
    }
    let file = std::fs::File::create(dest).map_err(|err| SyncUnitError::io(dest, err))?;
    let mut writer = HashingWriter {
        inner: file,
        hasher: Sha256::new(),
        len: 0,
        progress: ctx.progress,
        local_error: None,
    };
    let result = ctx.provider.read_to(remote_path, &mut writer);
    let local_error = writer.local_error.take();
    let (sha, len) = (to_hex(&writer.hasher.finalize()), writer.len);
    drop(writer.inner);
    let verdict = if ctx.progress.is_cancelled() {
        Err(SyncUnitError::Cancelled)
    } else if let Some(err) = local_error {
        Err(SyncUnitError::io(dest, err))
    } else {
        match result {
            Ok(_) if sha == expected_sha => return Ok(Some(len)),
            Ok(_) | Err(CloudError::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    };
    let _ = std::fs::remove_file(dest); // no partial / unverified file left behind
    verdict
}

/// Hashes and counts what is written; fails every write once the job is cancelled.
struct HashingWriter<'a, W> {
    inner: W,
    hasher: Sha256,
    len: u64,
    progress: &'a SyncProgress,
    /// A local disk error (told apart from provider errors).
    local_error: Option<io::Error>,
}

impl<W: Write> Write for HashingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.progress.is_cancelled() {
            return Err(io::Error::other("cloud sync cancelled"));
        }
        match self.inner.write(buf) {
            Ok(written) => {
                self.hasher.update(&buf[..written]);
                self.len += written as u64;
                Ok(written)
            }
            Err(err) => {
                self.local_error = Some(io::Error::new(err.kind(), err.to_string()));
                Err(err)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
