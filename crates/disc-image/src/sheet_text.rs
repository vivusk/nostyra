//! Reads a small text sheet (`.cue` / `.m3u`): size-capped (a mislabelled multi-MB
//! file is rejected, not loaded), UTF-8 BOM stripped, invalid UTF-8 replaced.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::DiscError;

/// Real sheets are a few KiB; anything bigger is not a sheet.
const MAX_SHEET_BYTES: u64 = 256 * 1024;

pub(crate) fn read_sheet_text(path: &Path) -> Result<String, DiscError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_SHEET_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SHEET_BYTES {
        return Err(DiscError::Format(
            "sheet file is too large to be a cue or m3u",
        ));
    }
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    Ok(String::from_utf8_lossy(text).into_owned())
}
