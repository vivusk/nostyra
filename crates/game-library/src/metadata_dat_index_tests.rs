//! Metadata index unit tests.

use super::*;

#[test]
fn version_matches_the_vendored_source_file() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../third-party/libretro-database/SOURCE.txt"),
    )
    .unwrap_or_default();
    let commit = source
        .lines()
        .find_map(|line| line.strip_prefix("Commit:"))
        .map(str::trim);
    assert_eq!(commit, Some(METADATA_SOURCE_VERSION));
}

#[test]
fn attributes_parse_numbers_and_skip_blanks() {
    let mut record = GameMetadataRecord::default();
    apply_attribute(&mut record, "releaseyear", "1995");
    apply_attribute(&mut record, "maxusers", "2");
    apply_attribute(&mut record, "genre", "  ");
    assert_eq!(record.release_year, Some(1995));
    assert_eq!(record.players, Some(2));
    assert_eq!(record.genre, None);
    apply_attribute(&mut record, "maxusers", "lots");
    assert_eq!(record.players, None);
}
