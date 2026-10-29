//! Chunked copy that enforces a byte cap and a cancel flag as it goes, so a
//! lying `Content-Length` or a zip bomb can never exhaust memory or disk.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{DownloadError, HttpResponse};

const CHUNK_BYTES: usize = 64 * 1024;

/// Copies `reader` into `writer` until EOF, failing with `TooLarge` as soon as
/// more than `limit` bytes arrive and with `Cancelled` when `cancel` is set.
/// `on_progress` gets the running byte count after each chunk.
/// `context` names the operation in I/O error messages.
pub fn copy_with_limit(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limit: u64,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64),
    context: &str,
) -> Result<u64, DownloadError> {
    let mut buffer = vec![0u8; CHUNK_BYTES];
    let mut copied: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        let read = match reader.read(&mut buffer) {
            Ok(0) => return Ok(copied),
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(DownloadError::io(context, error)),
        };
        copied += read as u64;
        if copied > limit {
            return Err(DownloadError::TooLarge { limit_bytes: limit });
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| DownloadError::io(context, error))?;
        on_progress(copied);
    }
}

/// Reads a whole response body into memory, refusing more than `limit` bytes
/// (checked against `Content-Length` first, then while streaming). A stream
/// that breaks mid-body (reset, body timeout) is `Transient`: nothing local
/// can fail when the sink is a `Vec`.
pub fn read_body_with_limit(
    response: HttpResponse,
    limit: u64,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, DownloadError> {
    let body = response.into_body();
    let announced = body.content_length();
    if announced.is_some_and(|length| length > limit) {
        return Err(DownloadError::TooLarge { limit_bytes: limit });
    }
    let capacity = usize::try_from(announced.unwrap_or(0)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    let mut reader = body.into_reader();
    match copy_with_limit(&mut reader, &mut bytes, limit, cancel, |_| {}, "body") {
        Ok(_) => Ok(bytes),
        Err(DownloadError::Io { source, .. }) => Err(DownloadError::Transient(format!(
            "download interrupted: {source}"
        ))),
        Err(other) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    fn copy(data: &[u8], limit: u64, cancel: bool) -> Result<Vec<u8>, DownloadError> {
        let mut out = Vec::new();
        let flag = AtomicBool::new(cancel);
        copy_with_limit(&mut &data[..], &mut out, limit, &flag, |_| {}, "test").map(|_| out)
    }

    #[test]
    fn copies_everything_up_to_the_limit() {
        assert_eq!(copy(b"abcd", 4, false).unwrap(), b"abcd");
    }

    #[test]
    fn stops_once_stream_exceeds_limit() {
        let data = vec![7u8; 3 * CHUNK_BYTES];
        assert!(matches!(
            copy(&data, CHUNK_BYTES as u64, false),
            Err(DownloadError::TooLarge { .. })
        ));
    }

    #[test]
    fn honours_cancel_flag() {
        assert!(matches!(
            copy(b"abcd", 4, true),
            Err(DownloadError::Cancelled)
        ));
    }

    #[test]
    fn reports_running_progress() {
        let data = vec![1u8; CHUNK_BYTES + 10];
        let mut seen = Vec::new();
        let flag = AtomicBool::new(false);
        let mut sink = std::io::sink();
        copy_with_limit(
            &mut &data[..],
            &mut sink,
            u64::MAX,
            &flag,
            |n| seen.push(n),
            "t",
        )
        .unwrap();
        assert_eq!(seen.last().copied(), Some(data.len() as u64));
    }
}
