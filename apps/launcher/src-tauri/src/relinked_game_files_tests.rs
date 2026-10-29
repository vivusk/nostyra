//! Save folder + game config names follow a renamed ROM; doubtful cases keep files in place.

use super::*;
use storage_db::{add_library_folder, open_in_memory_database, upsert_scanned_batch, ScannedGame};

struct Fixture {
    data: tempfile::TempDir,
    conn: Connection,
}

/// Content id of the game at index `i` (game id `i + 1`).
fn id(i: usize) -> String {
    format!("{:08X}", 0x1000_0000 + i)
}

/// One NES game at `E:/ROMs/<file>` (id 1), plus `others` with the same platform.
/// `same_content` = the others are copies of game 1 (same sha1).
fn fixture(file: &str, others: &[&str], same_content: bool) -> Fixture {
    let mut conn = open_in_memory_database().unwrap();
    let folder = add_library_folder(&conn, "E:/ROMs", 1).unwrap().id;
    let rows: Vec<ScannedGame> = std::iter::once(file)
        .chain(others.iter().copied())
        .enumerate()
        .map(|(i, name)| ScannedGame {
            platform_id: "nes".into(),
            folder_id: folder,
            rom_path: format!("E:/ROMs/{name}"),
            file_size: 1,
            file_mtime_ns: 0,
            crc32: format!("C{i}"),
            sha1: format!("{}{}", id(if same_content { 0 } else { i }), "0".repeat(32)),
            title: name.into(),
            region: None,
            dat_name: None,
            metadata: None,
            serial: None,
        })
        .collect();
    upsert_scanned_batch(&mut conn, &rows, 1).unwrap();
    let data = tempfile::Builder::new()
        .prefix("nostyra-relink-files-")
        .tempdir()
        .unwrap();
    Fixture { data, conn }
}

fn relink(old: &str, new: &str) -> RelinkedGame {
    RelinkedGame {
        game_id: 1,
        platform_id: "nes".into(),
        old_rom_path: format!("E:/ROMs/{old}"),
        new_rom_path: format!("E:/ROMs/{new}"),
    }
}

/// Writes `saves/nes/<name>/save.srm` and `config/game/nes/<name>.toml`.
fn write_game_files(data: &Path, name: &str) {
    let saves = data.join("saves/nes").join(name);
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("save.srm"), b"sram").unwrap();
    let config = data.join("config/game/nes");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join(format!("{name}.toml")), b"[video]").unwrap();
}

fn has_files(data: &Path, name: &str) -> bool {
    data.join("saves/nes").join(name).join("save.srm").is_file()
        && data.join(format!("config/game/nes/{name}.toml")).is_file()
}

fn run(f: &Fixture, old: &str, new: &str, running: Option<i64>) -> Vec<MigrationResult> {
    migrate_relinked_game_files(f.data.path(), &f.conn, &[relink(old, new)], running)
}

#[test]
fn rename_renames_id_named_files() {
    let f = fixture("Game (USA).nes", &[], false);
    write_game_files(f.data.path(), &format!("Game [{}]", id(0)));
    let results = run(&f, "Game.nes", "Game (USA).nes", None);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].moved.len(), 2, "{results:?}");
    assert!(results[0].skipped.is_empty());
    assert!(has_files(f.data.path(), &format!("Game (USA) [{}]", id(0))));
    assert!(!f
        .data
        .path()
        .join(format!("saves/nes/Game [{}]", id(0)))
        .exists());
}

#[test]
fn legacy_files_get_the_id_and_the_new_name() {
    let f = fixture("Game (USA).nes", &[], false);
    write_game_files(f.data.path(), "Game");
    let results = run(&f, "Game.nes", "Game (USA).nes", None);
    assert_eq!(results[0].moved.len(), 2, "{results:?}");
    assert!(has_files(f.data.path(), &format!("Game (USA) [{}]", id(0))));
    assert!(!f.data.path().join("saves/nes/Game").exists());
}

#[test]
fn user_chosen_name_and_folder_only_move_stay() {
    let f = fixture("New.nes", &[], false);
    let mine = format!("My Save [{}]", id(0));
    write_game_files(f.data.path(), &mine);
    assert!(run(&f, "Old.nes", "New.nes", None)[0].moved.is_empty());
    assert!(has_files(f.data.path(), &mine));

    let f = fixture("Game.nes", &[], false);
    let mut moved = relink("Game.nes", "Game.nes");
    moved.old_rom_path = "D:/Old/Game.nes".into();
    assert!(migrate_relinked_game_files(f.data.path(), &f.conn, &[moved], None).is_empty());
}

#[test]
fn shared_files_or_running_game_move_nothing() {
    // A copy of the same game elsewhere: both use the folder, its name stays.
    let f = fixture("New.nes", &["Sub/Old.nes"], true);
    let shared = format!("Old [{}]", id(0));
    write_game_files(f.data.path(), &shared);
    let results = run(&f, "Old.nes", "New.nes", None);
    assert!(results[0].moved.is_empty() && results[0].skipped.len() == 1);
    assert!(has_files(f.data.path(), &shared));

    // Another ROM named Old.nes: the legacy `Old` folder is left to the startup migration.
    let f = fixture("New.nes", &["Sub/Old.nes"], false);
    write_game_files(f.data.path(), "Old");
    assert!(run(&f, "Old.nes", "New.nes", None)[0].moved.is_empty());
    assert!(has_files(f.data.path(), "Old"));

    let f = fixture("New.nes", &[], false);
    write_game_files(f.data.path(), "Old");
    let results = run(&f, "Old.nes", "New.nes", Some(1));
    assert_eq!(results[0].skipped, ["the game is running"]);
    assert!(has_files(f.data.path(), "Old"));
}

#[test]
fn case_only_rename_never_loses_files() {
    let f = fixture("Game.nes", &[], false);
    write_game_files(f.data.path(), "game");
    run(&f, "game.nes", "Game.nes", None);
    let saves = f.data.path().join("saves/nes");
    let found = find_game_entry(&saves, &id(0), "").expect("found by id");
    assert!(found.join("save.srm").is_file());
}
