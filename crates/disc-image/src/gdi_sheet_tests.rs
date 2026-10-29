//! GDI parser tests: TOSEC lines, quoted names, CRLF and extra blanks, strictness.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;

const TOSEC: &str = "3\r\n1 0 4 2352 track01.bin 0\r\n2 756 0 2352 track02.raw 0\r\n3 45000 4 2352 track03.bin 0\r\n";

#[test]
fn parses_tosec_sheet() {
    let sheet = GdiSheet::parse(TOSEC).unwrap();
    assert_eq!(sheet.tracks.len(), 3);
    assert_eq!(
        sheet.tracks[2],
        GdiTrack {
            number: 3,
            lba: 45_000,
            data: true,
            sector_size: 2352,
            file: "track03.bin".into(),
            offset: 0,
        }
    );
    assert!(!sheet.tracks[1].data);
    assert_eq!(sheet.high_density_data_track().unwrap().number, 3);
}

#[test]
fn quoted_names_extra_blanks_and_2048_sectors() {
    let text = "2\n 1   0  4 2048  \"Crazy Taxi (Track 1).bin\"   0 \n2 45000 4 2048 \"Crazy Taxi (Track 2).bin\"\n";
    let sheet = GdiSheet::parse(text).unwrap();
    assert_eq!(sheet.tracks[0].file, "Crazy Taxi (Track 1).bin");
    assert_eq!(sheet.tracks[0].sector_size, 2048);
    assert_eq!(sheet.tracks[1].offset, 0, "offset column may be missing");
    let dir = Path::new("games");
    assert_eq!(
        sheet.referenced_paths(&dir.join("ct.gdi"))[1],
        dir.join("Crazy Taxi (Track 2).bin")
    );
}

#[test]
fn malformed_sheets_are_rejected() {
    for bad in [
        "",
        "abc\n1 0 4 2352 a.bin 0\n",
        "0\n",
        "2\n1 0 4 2352 a.bin 0\n",                      // count mismatch
        "1\n1 0 4 2000 a.bin 0\n",                      // sector size
        "1\n1 0 4 2352 \"a.bin 0\n",                    // unclosed quote
        "1\n1 0 4 2352\n",                              // no file
        "1\n1 0 4 2352 a.bin 0 extra\n",                // trailing words
        "FILE \"a.bin\" BINARY\nTRACK 01 MODE1/2352\n", // a cue
    ] {
        assert!(GdiSheet::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn missing_track_files_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("track01.bin"), b"x").unwrap();
    let gdi = dir.path().join("g.gdi");
    std::fs::write(&gdi, TOSEC).unwrap();
    let sheet = GdiSheet::read(&gdi).unwrap();
    assert_eq!(sheet.missing_files(&gdi), ["track02.raw", "track03.bin"]);
}

#[test]
fn sheet_without_high_density_track_has_no_boot_track() {
    let sheet = GdiSheet::parse("2\n1 0 4 2352 a.bin 0\n2 600 0 2352 b.raw 0\n").unwrap();
    assert!(sheet.high_density_data_track().is_none());
}
