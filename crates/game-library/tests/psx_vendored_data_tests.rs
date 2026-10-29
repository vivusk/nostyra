//! Vendored PlayStation data (Redump DAT + serial-keyed metadat) and a dev-only scan
//! of the developer's own discs.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::sync::atomic::AtomicBool;

use game_library::{scan_library, SystemCatalogs};
use storage_db::{list_games, GameListQuery};
use system_catalog::PSX;

#[test]
fn vendored_playstation_data_is_complete_and_serial_keyed() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let psx = catalogs.get(PSX.id);
    assert!(psx.dat.len() > 13_000);
    assert!(
        psx.metadata.is_complete(),
        "placeholders keep the index complete"
    );
    let ctr = psx
        .metadata
        .lookup_serial("SCUS-94426")
        .expect("CTR metadata");
    assert_eq!(ctr.developer.as_deref(), Some("Naughty Dog"));
    assert_eq!(ctr.genre.as_deref(), Some("Racing"));
    // Redump lists CTR twice with one SHA1 (SCUS-94426 and SCUS-94426CE): first wins.
    let by_sha1 = psx
        .dat
        .lookup_sha1("1BB785082DEF3FE25F6B46BE06D4EAF33C5BC772");
    assert_eq!(
        by_sha1.and_then(|e| e.serial.as_deref()),
        Some("SCUS-94426")
    );
    let (record, _) = catalogs
        .metadata_for_game(PSX.id, Some("CTR - Crash Team Racing (USA)"), "NOPE", None)
        .expect("self-heal by DAT serial");
    assert_eq!(record.developer.as_deref(), Some("Naughty Dog"));
}

/// Dev-only: scan the developer's own PS1 folder (read-only; temp database) and time
/// the first scan and the rescan. `cargo test -p game-library --test
/// psx_identification_tests -- --ignored --nocapture`.
#[test]
#[ignore = "needs the developer's own PS1 discs in E:/PSX"]
fn developer_psx_folder_scan() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-psx-dev-")
        .tempdir()
        .unwrap();
    let conn = storage_db::open_database(&temp.path().join("nostyra.db")).unwrap();
    let folder = storage_db::add_library_folder(&conn, "E:/PSX", 1).unwrap();
    let db = std::sync::Mutex::new(conn);
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    for pass in ["first", "rescan"] {
        let started = std::time::Instant::now();
        let summary = scan_library(
            &db,
            std::slice::from_ref(&folder),
            &catalogs,
            10,
            &|_| {},
            &AtomicBool::new(false),
        )
        .unwrap();
        println!("{pass}: {:?} ms {summary:?}", started.elapsed().as_millis());
    }
    let query = GameListQuery::default();
    for game in list_games(&db.lock().unwrap(), &query).unwrap() {
        println!(
            "{} | {:?} | {:?} | {:?} | {:?}",
            game.title, game.dat_name, game.serial, game.developer, game.rom_path
        );
    }
}
