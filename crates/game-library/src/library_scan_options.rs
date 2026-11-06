//! Persisted library discovery choices, independent of gameplay config layers.

use storage_db::{Connection, StorageResult};

/// Opt-in library discovery settings. Plain-ROM callers retain default behavior.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScanOptions {
    pub scan_zip_archives: bool,
}

const ZIP_SCAN_SETTING: &str = "scan_zip_archives";

pub fn read_scan_options(conn: &Connection) -> StorageResult<ScanOptions> {
    Ok(ScanOptions {
        scan_zip_archives: storage_db::get_setting(conn, ZIP_SCAN_SETTING)?.as_deref() == Some("1"),
    })
}

pub fn set_zip_scanning(conn: &Connection, enabled: bool) -> StorageResult<()> {
    storage_db::set_setting(conn, ZIP_SCAN_SETTING, if enabled { "1" } else { "0" })
}
