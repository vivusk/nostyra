//! Media downloads: ranged GETs of [`DOWNLOAD_RANGE_BYTES`], each with its own
//! timeout and retry, so a big blob is not bound by one request's time limit; a body
//! cut short resumes where it stopped. A file without a size gets one plain GET.

use std::io::{Read, Write};

use super::google_drive_provider::GoogleDriveProvider;
use super::google_drive_requests::{DriveCall, DriveFile, DRIVE_API};
use crate::CloudError;

/// One ranged media GET (same size as an upload chunk).
pub(crate) const DOWNLOAD_RANGE_BYTES: u64 = crate::http_transport::UPLOAD_CHUNK_BYTES;
/// A download whose ranges keep arriving empty is given up after this many tries.
const MAX_STALLED_RANGE_TRIES: u32 = 3;
const DOWNLOAD_BUFFER_BYTES: usize = 256 * 1024;

impl GoogleDriveProvider {
    /// Streams `file`'s content into `out`.
    pub(crate) fn download_to(
        &self,
        file: &DriveFile,
        out: &mut dyn Write,
    ) -> Result<(), CloudError> {
        let url = format!(
            "{DRIVE_API}/files/{}?alt=media",
            crate::encode_component(&file.id)
        );
        let Some(size) = file.size() else {
            let mut reply = self.send(&DriveCall::get(url))?;
            return copy_body(&mut reply.body, out, u64::MAX, &mut 0);
        };
        let (mut offset, mut stalls) = (0u64, 0u32);
        while offset < size {
            let end = size.min(offset + DOWNLOAD_RANGE_BYTES);
            let range = format!("bytes={offset}-{}", end - 1);
            let extra = [("Range", range.as_str())];
            let mut call = DriveCall::get(url.clone());
            call.extra_headers = &extra;
            call.reply_bytes = end - offset;
            let mut reply = self.send(&call)?;
            let limit = match reply.status {
                206 => end - offset,
                _ if offset == 0 => size, // the whole file despite the Range
                _ => return Err(CloudError::Other("drive ignored the download range".into())),
            };
            let before = offset;
            match copy_body(&mut reply.body, out, limit, &mut offset) {
                Ok(()) | Err(CloudError::Offline) => {}
                Err(err) => return Err(err),
            }
            stalls = if offset > before { 0 } else { stalls + 1 };
            if stalls > MAX_STALLED_RANGE_TRIES {
                return Err(CloudError::Offline);
            }
        }
        Ok(())
    }
}

/// Copies at most `limit` body bytes into `out`, counting them in `offset`. A body
/// read error (connection cut) is `Offline`, a writer error (cancel) aborts.
fn copy_body(
    body: &mut dyn Read,
    out: &mut dyn Write,
    limit: u64,
    offset: &mut u64,
) -> Result<(), CloudError> {
    let mut body = body.take(limit);
    let mut buffer = vec![0u8; DOWNLOAD_BUFFER_BYTES];
    loop {
        let read = body.read(&mut buffer).map_err(|_| CloudError::Offline)?;
        if read == 0 {
            return Ok(());
        }
        out.write_all(&buffer[..read])
            .map_err(|err| CloudError::Other(format!("download aborted: {err}")))?;
        *offset += read as u64;
    }
}
