//! The ZIP scan opt-in survives reopening the library and defaults off.
#![allow(clippy::unwrap_used)]

use game_library::{read_scan_options, set_zip_scanning};

#[test]
fn zip_scan_opt_in_defaults_off_and_persists_both_values() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.db");
    {
        let db = storage_db::open_database(&path).unwrap();
        assert!(!read_scan_options(&db).unwrap().scan_zip_archives);
        set_zip_scanning(&db, true).unwrap();
    }
    {
        let db = storage_db::open_database(&path).unwrap();
        assert!(read_scan_options(&db).unwrap().scan_zip_archives);
        set_zip_scanning(&db, false).unwrap();
    }
    let db = storage_db::open_database(&path).unwrap();
    assert!(!read_scan_options(&db).unwrap().scan_zip_archives);
}
