//! M2 exit criterion guard: the vendored metadat covers the bundled No-Intro DAT
//! (Detail shows metadata for ≥ 85 % of matched games).
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::collections::HashSet;
use std::path::Path;

use game_library::MetadataIndex;

#[test]
fn vendored_metadat_covers_the_bundled_dat() {
    let db_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let dat_text = std::fs::read_to_string(db_dir.join(system_catalog::SNES.dat_file)).unwrap();
    let crcs: HashSet<String> = game_library::parse_clrmamepro_dat(&dat_text)
        .unwrap()
        .into_iter()
        .map(|entry| entry.crc32)
        .collect();
    assert!(crcs.len() > 3_000, "bundled DAT parsed");

    let index = MetadataIndex::load(&db_dir.join("metadat"), system_catalog::SNES.metadat_file);
    assert!(index.is_complete(), "all five metadat files load");
    let share = |field: &dyn Fn(&storage_db::GameMetadataRecord) -> bool| {
        let hits = crcs
            .iter()
            .filter(|crc| index.lookup(crc).is_some_and(field))
            .count();
        hits as f64 / crcs.len() as f64 * 100.0
    };
    let developer = share(&|r| r.developer.is_some());
    let genre = share(&|r| r.genre.is_some());
    let year = share(&|r| r.release_year.is_some());
    let players = share(&|r| r.players.is_some());
    eprintln!(
        "metadat coverage of {} DAT CRCs: developer {developer:.1} %, genre {genre:.1} %, \
         year {year:.1} %, players {players:.1} %",
        crcs.len()
    );
    assert!(developer >= 85.0, "developer {developer:.1} %");
    assert!(genre >= 85.0, "genre {genre:.1} %");
}
