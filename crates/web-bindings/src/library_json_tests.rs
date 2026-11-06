use std::rc::Rc;

use content_file_source::{with_source, MemoryFiles};

use super::*;

fn test_asset(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test-assets/roms")
            .join(name),
    )
    .unwrap()
}

#[test]
fn metadata_from_texts_matches_the_desktop_lookup() {
    let genre = "game (\n\tname \"Chrono Trigger (USA)\"\n\tgenre \"Role-Playing (RPG)\"\n\trom ( crc 2D5CE2BA )\n)\n";
    let year = "game (\n\tname \"Chrono Trigger (USA)\"\n\treleaseyear \"1995\"\n\trom ( crc 2D5CE2BA )\n)\n";
    let texts = [None, None, Some(genre.into()), Some(year.into()), None];
    let index = metadata_index("snes", texts).unwrap();
    assert!(!index.is_complete(), "three files missing");
    let view = stored_metadata(&DatIndex::empty(), &index, None, "2d5ce2ba", None).unwrap();
    assert_eq!(view.genre.as_deref(), Some("Role-Playing (RPG)"));
    assert_eq!(view.release_year, Some(1995));
    assert_eq!(view.source_version, rom_identity::METADATA_PARTIAL_VERSION);
    assert!(metadata_index("nope", Default::default()).is_err());
}

#[test]
fn groups_a_registered_playlist_like_the_desktop() {
    let mut files = MemoryFiles::new();
    for name in [
        "nostyra-test-psx.m3u",
        "nostyra-test-psx.cue",
        "nostyra-test-psx.bin",
        "nostyra-test-psx-disc2.cue",
        "nostyra-test-psx-disc2.bin",
    ] {
        files.insert(format!("/web/1/{name}"), test_asset(name));
    }
    let walk: Vec<GroupFile> = [
        "nostyra-test-psx.m3u",
        "nostyra-test-psx.cue",
        "nostyra-test-psx.bin",
        "nostyra-test-psx-disc2.cue",
    ]
    .iter()
    .map(|name| GroupFile {
        path: format!("/web/1/{name}"),
        disc: true,
    })
    .collect();
    let failures = vec!["/web/1/nostyra-test-psx-disc2.bin".to_string()];
    let grouping = with_source(Rc::new(files), || group_files(&walk, &failures));
    assert_eq!(grouping.hidden_files, [1, 2, 3], "only the playlist stays");
    assert_eq!(grouping.hidden_failures, [0]);
    assert_eq!(grouping.members.len(), 1);
    assert_eq!(grouping.members[0].index, 0);
    assert_eq!(
        grouping.members[0].paths.len(),
        4,
        "both cues and both bins"
    );
}
