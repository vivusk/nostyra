//! Size cap of [`crate::CloudStorageProvider::read`]: whole-file reads are only for
//! small metadata JSON (`manifest.json`, `index.json`, `format.json`), so a huge or
//! endless file in a shared / corrupted cloud folder is refused before it is held in
//! memory (treated like a corrupt file). Blobs stream through `read_to`, uncapped.

use std::io::{self, Read, Write};
use std::path::Path;

use crate::cloud_error::CloudError;

/// Largest metadata file `read` accepts (real manifests / indexes are a few KiB).
pub const MAX_METADATA_BYTES: u64 = 8 * 1024 * 1024;

/// The error of an oversized metadata file.
pub(crate) fn metadata_too_large(path: &str) -> CloudError {
    CloudError::Other(format!(
        "{path}: larger than {} MiB, treated as corrupt",
        MAX_METADATA_BYTES / (1024 * 1024)
    ))
}

/// Reads a local metadata file, at most [`MAX_METADATA_BYTES`] (else an
/// `io::Error` whose message names the cap; `NotFound` etc. pass through).
pub(crate) fn read_file_capped(file: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(file)?
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err(io::Error::other(format!(
            "metadata file larger than {} MiB, treated as corrupt",
            MAX_METADATA_BYTES / (1024 * 1024)
        )));
    }
    Ok(bytes)
}

/// An in-memory download target that refuses bytes past [`MAX_METADATA_BYTES`]
/// (the write error aborts the download).
#[derive(Default)]
pub(crate) struct CappedBuffer {
    bytes: Vec<u8>,
    overflowed: bool,
}

impl CappedBuffer {
    /// The bytes of a finished download; oversized → [`metadata_too_large`] even
    /// when the download itself reported the aborted write differently.
    pub(crate) fn finish(
        self,
        download: Result<(), CloudError>,
        path: &str,
    ) -> Result<Vec<u8>, CloudError> {
        if self.overflowed {
            return Err(metadata_too_large(path));
        }
        download.map(|()| self.bytes)
    }
}

impl Write for CappedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if (self.bytes.len() + buf.len()) as u64 > MAX_METADATA_BYTES {
            self.overflowed = true;
            return Err(io::Error::other("metadata file too large"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_buffer_refuses_past_the_cap() {
        let mut buf = CappedBuffer::default();
        buf.write_all(&vec![1u8; MAX_METADATA_BYTES as usize])
            .unwrap();
        assert!(buf.write_all(&[1]).is_err());
        let err = buf.finish(Ok(()), "games/x/manifest.json").unwrap_err();
        assert!(matches!(err, CloudError::Other(msg) if msg.contains("8 MiB")));

        let mut small = CappedBuffer::default();
        small.write_all(b"{}").unwrap();
        assert_eq!(small.finish(Ok(()), "index.json").unwrap(), b"{}");
    }

    #[test]
    fn capped_file_read_refuses_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let exact = dir.path().join("exact.json");
        std::fs::write(&exact, vec![b' '; MAX_METADATA_BYTES as usize]).unwrap();
        assert_eq!(
            read_file_capped(&exact).unwrap().len() as u64,
            MAX_METADATA_BYTES
        );
        let big = dir.path().join("big.json");
        std::fs::write(&big, vec![b' '; MAX_METADATA_BYTES as usize + 1]).unwrap();
        assert!(read_file_capped(&big).is_err());
        let missing = read_file_capped(&dir.path().join("none")).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    }
}
