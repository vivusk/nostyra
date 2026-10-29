//! Cue parser tests: Redump multi-track layout, keyword case, quoting, missing files.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;

const REDUMP: &str = "\u{feff}REM generated\r\nFILE \"Game (Track 1).bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\nFILE \"Game (Track 2).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n    INDEX 00 00:00:00\r\n    INDEX 01 00:02:00\r\n";

#[test]
fn parses_redump_multi_track_sheet() {
    let sheet = CueSheet::parse(REDUMP.trim_start_matches('\u{feff}'));
    assert_eq!(sheet.files.len(), 2);
    assert_eq!(sheet.files[0].name, "Game (Track 1).bin");
    assert_eq!(
        sheet.files[0].tracks,
        vec![CueTrack {
            number: 1,
            mode: TrackMode::Mode2Raw,
            area: TrackArea::Single,
            index01: Some(0)
        }]
    );
    assert_eq!(sheet.files[1].tracks[0].mode, TrackMode::Audio);
    assert_eq!(sheet.files[1].tracks[0].index01, Some(150));
}

#[test]
fn keywords_are_case_insensitive_and_names_may_be_unquoted() {
    let sheet =
        CueSheet::parse("file game data.iso binary\ntrack 1 mode1/2048\nindex 1 00:00:00\n");
    assert_eq!(sheet.files[0].name, "game data.iso");
    assert_eq!(sheet.files[0].tracks[0].mode, TrackMode::Mode1Cooked);
    let motorola = CueSheet::parse("FILE \"a.bin\" MOTOROLA\nTRACK 01 MODE1/2352\n");
    assert_eq!(motorola.files[0].tracks[0].mode, TrackMode::Mode1Raw);
}

#[test]
fn malformed_lines_are_skipped() {
    let sheet = CueSheet::parse("TRACK 01 MODE2/2352\nFILE \"x.bin\" BINARY\nTRACK xx AUDIO\nINDEX 01 99:99:99\nTRACK 02 MODE2/2336\n");
    assert_eq!(sheet.files.len(), 1);
    assert_eq!(sheet.files[0].tracks.len(), 1);
    assert_eq!(sheet.files[0].tracks[0].mode, TrackMode::Other);
    assert_eq!(sheet.files[0].tracks[0].index01, None);
}

#[test]
fn reports_missing_files_like_a_broken_redump_rename() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Game (Track 2).bin"), [0u8; 4]).unwrap();
    let cue = dir.path().join("Game.cue");
    std::fs::write(&cue, REDUMP).unwrap();
    let sheet = CueSheet::read(&cue).unwrap();
    assert_eq!(
        sheet.missing_files(&cue),
        vec!["Game (Track 1).bin".to_owned()]
    );
    assert_eq!(
        sheet.referenced_paths(&cue)[1],
        dir.path().join("Game (Track 2).bin")
    );
    let (track, entry) = sheet.first_data_track(&cue).unwrap();
    let mode = entry.mode;
    assert_eq!(
        (track, mode),
        (dir.path().join("Game (Track 1).bin"), TrackMode::Mode2Raw)
    );
}

#[test]
fn audio_only_sheet_has_no_data_track() {
    let sheet = CueSheet::parse("FILE \"a.bin\" BINARY\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n");
    assert!(matches!(
        sheet.first_data_track(Path::new("a.cue")),
        Err(DiscError::NoDataTrack)
    ));
}

#[test]
fn oversized_sheet_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let cue = dir.path().join("big.cue");
    std::fs::write(&cue, vec![b' '; 300 * 1024]).unwrap();
    assert!(matches!(CueSheet::read(&cue), Err(DiscError::Format(_))));
}

/// Redump Dreamcast layout: track 1 data + track 2 audio in the single-density area,
/// track 3 data (IP.BIN) + more tracks in the high-density area.
const REDUMP_GD: &str = "FILE \"G (Track 1).bin\" BINARY
  TRACK 01 MODE1/2352
    INDEX 01 00:00:00
FILE \"G (Track 2).bin\" BINARY
  TRACK 02 AUDIO
    INDEX 00 00:00:00
    INDEX 01 00:02:00
REM HIGH-DENSITY AREA
FILE \"G (Track 3).bin\" BINARY
  TRACK 03 MODE1/2352
    INDEX 01 00:00:00
FILE \"G (Track 4).bin\" BINARY
  TRACK 04 MODE1/2352
    INDEX 00 00:00:00
    INDEX 01 00:03:00
";

#[test]
fn density_area_markers_tag_tracks_and_pick_the_boot_track() {
    let text = format!(
        "REM SINGLE-DENSITY AREA
{REDUMP_GD}"
    );
    let sheet = CueSheet::parse(&text);
    let areas: Vec<TrackArea> = sheet.files.iter().map(|f| f.tracks[0].area).collect();
    use TrackArea::{High, Single};
    assert_eq!(areas, [Single, Single, High, High]);
    let cue = Path::new("dir/g.cue");
    let (file, track) = sheet.boot_data_track(cue).unwrap();
    assert_eq!(
        (file, track.number),
        (Path::new("dir").join("G (Track 3).bin"), 3)
    );
    assert_eq!(sheet.first_data_track(cue).unwrap().1.number, 1);
    let numbers: Vec<u32> = sheet
        .data_tracks(cue)
        .iter()
        .map(|(_, t)| t.number)
        .collect();
    assert_eq!(numbers, [1, 3, 4]);
}

#[test]
fn cd_sheets_have_no_high_area_and_boot_from_the_first_data_track() {
    let sheet = CueSheet::parse(REDUMP);
    assert!(sheet
        .files
        .iter()
        .flat_map(|f| &f.tracks)
        .all(|t| t.area == TrackArea::Single));
    assert_eq!(
        sheet.boot_data_track(Path::new("g.cue")).unwrap().1.number,
        1
    );
}
