//! m3u tests: comments, relative and absolute entries, limits.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;

#[test]
fn skips_comments_and_resolves_relative_entries() {
    let dir = Path::new("/games/ff7");
    let abs = if cfg!(windows) {
        "C:\\discs\\d3.cue"
    } else {
        "/discs/d3.cue"
    };
    let text = format!("#EXTM3U\n\nFF7 (Disc 1).cue\r\n  sub/FF7 (Disc 2).chd  \n{abs}\n");
    let entries = parse_playlist(&text, dir).unwrap();
    assert_eq!(
        entries,
        vec![
            dir.join("FF7 (Disc 1).cue"),
            dir.join("sub/FF7 (Disc 2).chd"),
            PathBuf::from(abs)
        ]
    );
}

#[test]
fn rejects_empty_oversized_and_nested_playlists() {
    let dir = Path::new("d");
    assert!(matches!(
        parse_playlist("# only a comment\n", dir),
        Err(DiscError::EmptyPlaylist)
    ));
    let nine: String = (1..=9).map(|i| format!("d{i}.cue\n")).collect();
    assert!(matches!(
        parse_playlist(&nine, dir),
        Err(DiscError::TooManyDiscs(9))
    ));
    let eight: String = (1..=8).map(|i| format!("d{i}.cue\n")).collect();
    assert_eq!(parse_playlist(&eight, dir).unwrap().len(), 8);
    let err = parse_playlist("a.cue\nother.M3U\n", dir).unwrap_err();
    assert_eq!(
        err.to_string(),
        "playlist entry 'other.M3U' is itself a playlist"
    );
}

#[test]
fn reads_playlist_file_with_bom() {
    let dir = tempfile::tempdir().unwrap();
    let m3u = dir.path().join("Game.m3u");
    std::fs::write(&m3u, "\u{feff}Game (Disc 1).cue\nGame (Disc 2).cue\n").unwrap();
    let entries = playlist_entries(&m3u).unwrap();
    assert_eq!(entries[0], dir.path().join("Game (Disc 1).cue"));
    assert_eq!(entries.len(), 2);
}
