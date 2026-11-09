//! Big-file uploads: a resumable session fed 5 MiB chunks (a multiple of 256 KiB);
//! after a transient chunk failure the session is asked how far it got (`308` +
//! `Range`) and the upload resumes there. The current chunk stays in memory, so a
//! resume never re-reads the source; an expired session (404 / 410) cannot rewind
//! the stream → `Offline`, and the next sync restarts the upload with a fresh session.

use std::io::Read;

use http_download_util::{HttpMethod, HttpRequest, RequestBody};

use super::google_drive_errors::drive_error;
use super::google_drive_provider::GoogleDriveProvider;
use super::google_drive_requests::{DriveCall, DriveFile, JSON_TYPE};
use super::google_drive_uploads::{rev_of, UploadTarget};
use crate::http_transport::{
    read_body, with_retry, CONNECT_TIMEOUT, MAX_JSON_REPLY_BYTES, UPLOAD_CHUNK_BYTES,
};
use crate::{CloudError, Rev};

/// A session that keeps acknowledging no new bytes is given up after this many tries.
const MAX_STALLED_CHUNK_TRIES: u32 = 3;

impl GoogleDriveProvider {
    /// Resumable upload of exactly `len` bytes from `reader` into the open `session`.
    pub(crate) fn upload_resumable(
        &self,
        session: &str,
        reader: &mut dyn Read,
        len: u64,
    ) -> Result<Rev, CloudError> {
        let mut chunk = vec![0u8; UPLOAD_CHUNK_BYTES.min(len) as usize];
        let mut offset = 0u64;
        while offset < len {
            let size = UPLOAD_CHUNK_BYTES.min(len - offset) as usize;
            read_full(reader, &mut chunk[..size])?;
            let end = offset + size as u64;
            let mut acked = offset;
            let (mut ask_first, mut stalls) = (false, 0);
            while acked < end {
                let pending = &chunk[(acked - offset) as usize..size];
                let range = format!("bytes {acked}-{}/{len}", end - 1);
                let step =
                    with_retry(self.session.transport(), self.budget.retry_policy(), || {
                        let result = if ask_first {
                            self.chunk_step(session, &format!("bytes */{len}"), &[])
                        } else {
                            self.chunk_step(session, &range, pending)
                        };
                        ask_first = matches!(&result, Err(err) if err.is_transient());
                        result
                    })?;
                match step {
                    ChunkStep::Done(rev) if end == len => return Ok(rev),
                    ChunkStep::Done(_) => {
                        return Err(CloudError::Other("upload ended early".into()))
                    }
                    ChunkStep::Expired => return Err(CloudError::Offline),
                    ChunkStep::Acked(next) if next < offset || next > end => {
                        return Err(CloudError::Offline)
                    }
                    ChunkStep::Acked(next) => {
                        stalls = if next > acked { 0 } else { stalls + 1 };
                        if stalls > MAX_STALLED_CHUNK_TRIES {
                            return Err(CloudError::Offline);
                        }
                        acked = next;
                    }
                }
            }
            offset = end;
        }
        Err(CloudError::Other(
            "upload loop ended without a final reply".into(),
        ))
    }

    /// Opens the session; its URL (the `Location` header) is the upload credential.
    pub(crate) fn start_session(
        &self,
        target: &UploadTarget<'_>,
        len: u64,
    ) -> Result<String, CloudError> {
        let (method, url) = target.method_and_url("resumable");
        let metadata = target.metadata();
        let length = len.to_string();
        let extra = [
            ("X-Upload-Content-Type", "application/octet-stream"),
            ("X-Upload-Content-Length", length.as_str()),
        ];
        let mut call = DriveCall::new(method, url, Some(JSON_TYPE), metadata.as_bytes(), false);
        call.extra_headers = &extra;
        let reply = self.send(&call)?;
        reply
            .header("location")
            .map(str::to_string)
            .ok_or_else(|| CloudError::Other("upload session without a location".into()))
    }

    /// One `PUT` to the session (a chunk, or `bytes */len` = "how far are you?").
    fn chunk_step(
        &self,
        session: &str,
        range: &str,
        bytes: &[u8],
    ) -> Result<ChunkStep, CloudError> {
        let reply = self.session.transport().send(HttpRequest {
            method: HttpMethod::Put,
            url: session,
            headers: &[("Content-Range", range)],
            body: RequestBody::Bytes(bytes),
            connect_timeout: Some(CONNECT_TIMEOUT),
            timeout: Some(self.budget.transfer_timeout(bytes.len() as u64)?),
        })?;
        match reply.status {
            308 => Ok(ChunkStep::Acked(acked_end(reply.header("range")))),
            200 | 201 => {
                let body = read_body(reply, MAX_JSON_REPLY_BYTES)?;
                let file: DriveFile = serde_json::from_slice(&body)
                    .map_err(|_| CloudError::Other("unreadable drive reply".into()))?;
                Ok(ChunkStep::Done(rev_of(&file)?))
            }
            404 | 410 => Ok(ChunkStep::Expired),
            _ => Err(drive_error(reply)),
        }
    }
}

enum ChunkStep {
    Acked(u64),
    Done(Rev),
    Expired,
}

/// `Range: bytes=0-<last>` → the next offset (`last + 1`); none = nothing stored.
fn acked_end(range: Option<&str>) -> u64 {
    range
        .and_then(|value| value.rsplit('-').next())
        .and_then(|last| last.trim().parse::<u64>().ok())
        .map_or(0, |last| last + 1)
}

/// Fills `buf` from `reader`; a short stream or a reader error (cancel) aborts.
fn read_full(reader: &mut dyn Read, buf: &mut [u8]) -> Result<(), CloudError> {
    reader
        .read_exact(buf)
        .map_err(|err| CloudError::Other(format!("upload source: {err}")))
}
