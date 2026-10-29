use super::*;

const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nostyra-game-key-")
        .tempdir()
        .unwrap()
}

#[test]
fn content_id_is_upper_hex_prefix() {
    assert_eq!(content_id(SHA1).as_deref(), Some("6B47BB75"));
    assert_eq!(content_id(""), None);
    assert_eq!(content_id("6b47"), None);
    assert_eq!(content_id("zzzzzzzz00"), None);
}

#[test]
fn id_is_read_from_the_name_suffix() {
    assert_eq!(
        content_id_of_name("Game (USA) [6B47BB75]", ""),
        Some("6B47BB75")
    );
    assert_eq!(
        content_id_of_name("Game [6b47bb75].TOML", ".toml"),
        Some("6b47bb75")
    );
    assert_eq!(content_id_of_name("Game [6B47BB75].toml", ""), None);
    assert_eq!(content_id_of_name("Game (USA) [!]", ""), None);
    assert_eq!(content_id_of_name("[6B47BB7]", ""), None);
    assert_eq!(
        content_id_of_name("ドラゴン [6B47BB75]", ""),
        Some("6B47BB75")
    );
}

#[test]
fn new_entry_gets_readable_name_and_id() {
    let temp = scratch();
    let dir = resolve_game_entry(temp.path(), Path::new("E:/ROMs/Zelda: Link?.sfc"), SHA1, "");
    assert_eq!(dir, temp.path().join("Zelda_ Link_ [6B47BB75]"));
    assert!(!dir.exists(), "resolving never creates the entry");
}

#[test]
fn found_by_id_after_rom_and_folder_renames() {
    let temp = scratch();
    let kept = temp.path().join("My own name [6b47bb75]");
    std::fs::create_dir(&kept).unwrap();
    std::fs::create_dir(temp.path().join("Other [00000000]")).unwrap();
    let dir = resolve_game_entry(temp.path(), Path::new("Renamed.sfc"), SHA1, "");
    assert_eq!(dir, kept);
}

#[test]
fn legacy_entry_is_renamed_once() {
    let temp = scratch();
    let legacy = temp.path().join("Game.toml");
    std::fs::write(&legacy, b"x").unwrap();
    let file = resolve_game_entry(temp.path(), Path::new("roms/Game.sfc"), SHA1, ".toml");
    assert_eq!(file, temp.path().join("Game [6B47BB75].toml"));
    assert!(file.is_file() && !legacy.exists());
    // Second resolve finds it by id; a new legacy-named file is left alone.
    std::fs::write(&legacy, b"y").unwrap();
    assert_eq!(
        resolve_game_entry(temp.path(), Path::new("roms/Game.sfc"), SHA1, ".toml"),
        file
    );
    assert!(legacy.exists());
}

#[test]
fn no_usable_sha1_keeps_the_legacy_name() {
    let temp = scratch();
    assert_eq!(
        resolve_game_entry(temp.path(), Path::new("Game.sfc"), "", ""),
        temp.path().join("Game")
    );
}
