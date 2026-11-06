//! Missing-game removal against scratch data folders.

use super::*;

fn scratch(name: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-remove-missing-{name}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"x").unwrap();
}

const SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

fn plan(key_shared: bool) -> RemovalPlan {
    RemovalPlan {
        system: "snes".into(),
        rom_path: "E:/ROMs/Chrono Trigger.sfc".into(),
        content_sha1: SHA1.into(),
        artwork_files: vec!["7-cover.jpg".into()],
        key_shared,
    }
}

#[test]
fn deletes_saves_config_and_art() {
    let temp = scratch("all");
    let root = temp.path();
    let art = root.join("artwork");
    let p = plan(false);
    let saves = ipc_protocol::saves_dir_for_game(root, "snes", &p.rom_path, SHA1);
    let config = ipc_protocol::game_config_path_for_game(root, "snes", &p.rom_path, SHA1);
    touch(&saves.join("save.srm"));
    touch(&saves.join("slot1.state"));
    // The web edition's files belong to the game too and go with its folder.
    for name in [
        "web-slot1.state",
        "web-auto.state",
        "save.web.srm",
        "save.srm.crswap",
    ] {
        touch(&saves.join(name));
    }
    touch(&config);
    touch(&art.join("7-cover.jpg"));
    touch(&art.join("8-cover.jpg"));
    let legacy = root.join("saves/snes/Chrono Trigger");
    touch(&legacy.join("save.srm"));

    p.delete_files(root, &art).unwrap();
    assert!(!saves.exists() && !config.exists() && !art.join("7-cover.jpg").exists());
    assert!(art.join("8-cover.jpg").exists(), "other games' art stays");
    assert!(
        legacy.join("save.srm").exists(),
        "legacy folders are never deleted"
    );
    // Nothing left to delete is not an error.
    p.delete_files(root, &art).unwrap();
}

#[test]
fn shared_content_id_keeps_saves_and_config() {
    let temp = scratch("shared");
    let root = temp.path();
    let art = root.join("artwork");
    let p = plan(true);
    let saves = ipc_protocol::saves_dir_for_game(root, "snes", &p.rom_path, SHA1);
    let config = ipc_protocol::game_config_path_for_game(root, "snes", &p.rom_path, SHA1);
    touch(&saves.join("save.srm"));
    touch(&config);

    p.delete_files(root, &art).unwrap();
    assert!(saves.join("save.srm").exists() && config.exists());
}
