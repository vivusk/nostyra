//! Uploading one staged file as a blob while hashing exactly the bytes sent: the
//! streamed sha must equal the planned (snapshot) sha, else a core rewrote the file
//! in place after staging and the push is aborted (no manifest). Big files go
//! through `write_large` with a reader that also stops on cancel.

use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::content_hash::{sha256_hex, to_hex};
use crate::sync_outcome::{SyncContext, SyncUnitError};
use crate::sync_progress::SyncProgress;

/// Files above this go through `write_large` (chunked upload).
pub const LARGE_WRITE_BYTES: u64 = 4 * 1024 * 1024;

/// Uploads `staged` to `remote_path`. `Some(size)` when the bytes sent hash to
/// `planned_sha`, `None` when they do not (the blob, if written, stays orphaned:
/// never referenced, collected later).
pub(crate) fn upload_staged(
    ctx: &SyncContext<'_>,
    staged: &Path,
    remote_path: &str,
    planned_sha: &str,
) -> Result<Option<u64>, SyncUnitError> {
    let len = std::fs::metadata(staged)
        .map_err(|err| SyncUnitError::io(staged, err))?
        .len();
    let sent_sha = if len > LARGE_WRITE_BYTES {
        let file = std::fs::File::open(staged).map_err(|err| SyncUnitError::io(staged, err))?;
        let mut reader = HashingReader::new(file, ctx.progress);
        let result = ctx.provider.write_large(remote_path, &mut reader, len);
        if ctx.progress.is_cancelled() {
            return Err(SyncUnitError::Cancelled);
        }
        result?;
        reader.finish()
    } else {
        let bytes = std::fs::read(staged).map_err(|err| SyncUnitError::io(staged, err))?;
        let sha = sha256_hex(&bytes);
        if sha != planned_sha {
            return Ok(None); // changed already: do not upload
        }
        ctx.provider.write(remote_path, &bytes, None)?;
        sha
    };
    Ok((sent_sha == planned_sha).then_some(len))
}

/// Hashes what is read; fails every read once the job is cancelled.
struct HashingReader<'a, R> {
    inner: R,
    hasher: Sha256,
    progress: &'a SyncProgress,
}

impl<'a, R: Read> HashingReader<'a, R> {
    fn new(inner: R, progress: &'a SyncProgress) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            progress,
        }
    }

    fn finish(self) -> String {
        to_hex(&self.hasher.finalize())
    }
}

impl<R: Read> Read for HashingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.progress.is_cancelled() {
            // Not `Interrupted`: `read_to_end` / `io::copy` would retry that.
            return Err(io::Error::other("cloud sync cancelled"));
        }
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}
