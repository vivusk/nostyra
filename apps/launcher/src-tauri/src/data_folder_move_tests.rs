//! Data folder move: only Nostyra entries move, conflicts, vanished files, overlap.

use super::*;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn moves_only_nostyra_entries_and_overwrites_on_request() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    write(from.path(), "nostyra.db", "old db");
    write(from.path(), "saves/snes/Zelda [AB]/save.srm", "old sram");
    write(from.path(), "config/global.toml", "old config");
    write(from.path(), "notes.txt", "not ours");
    write(to.path(), "config/global.toml", "new config");

    let plan = plan_move(from.path(), to.path()).unwrap();
    assert_eq!(plan.files.len(), 3);
    assert_eq!(plan.conflicts, 1);

    let mut last = (0, 0);
    let copied =
        copy_planned_files(from.path(), to.path(), &plan, true, |d, t| last = (d, t)).unwrap();
    assert_eq!(copied.len(), 3);
    assert_eq!(last, (3, 3));
    assert_eq!(read(to.path(), "config/global.toml"), "old config");
    assert_eq!(
        read(to.path(), "saves/snes/Zelda [AB]/save.srm"),
        "old sram"
    );

    remove_moved_originals(from.path(), &plan, &copied);
    assert!(!from.path().join("nostyra.db").exists());
    assert!(
        !from.path().join("saves").exists(),
        "emptied folders are removed"
    );
    assert_eq!(read(from.path(), "notes.txt"), "not ours");
    assert!(!to.path().join("notes.txt").exists());
}

#[test]
fn achievement_files_and_badge_cache_move_too() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    write(from.path(), "achievements/credentials.json", "{}");
    write(from.path(), "cache/achievement-badges/00234.png", "png");
    let plan = plan_move(from.path(), to.path()).unwrap();
    assert_eq!(plan.files.len(), 2);
    copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(read(to.path(), "cache/achievement-badges/00234.png"), "png");
}

/// Cloud saves keep the account, device id, base manifests and backups with the data.
#[test]
fn cloud_folder_moves_too() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    assert!(DATA_ENTRIES.contains(&"cloud"));
    assert_eq!(DATA_ENTRIES.len(), 14);
    write(from.path(), "cloud/device.json", "{}");
    write(
        from.path(),
        "cloud/base/local_folder-0123/snes/abc.json",
        "{}",
    );
    let plan = plan_move(from.path(), to.path()).unwrap();
    assert_eq!(plan.files.len(), 2);
    copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(read(to.path(), "cloud/device.json"), "{}");
}

/// PlayStation 2 keeps its BIOS (+ the core's `.nvm` / `.mec`) and the shared memory
/// cards under `system/pcsx2/`, outside `saves/`: the `system` entry carries them.
#[test]
fn ps2_bios_sidecars_and_shared_memory_cards_move_with_the_system_folder() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    let files = [
        "system/pcsx2/bios/scph39001.bin",
        "system/pcsx2/bios/scph39001.nvm",
        "system/pcsx2/bios/scph39001.mec",
        "system/pcsx2/memcards/Mcd001.ps2",
        "system/pcsx2/memcards/Mcd002.ps2",
        "saves/ps2/Game [FA7546D5]/slot1.state",
    ];
    for rel in files {
        write(from.path(), rel, rel);
    }
    let plan = plan_move(from.path(), to.path()).unwrap();
    assert_eq!(plan.files.len(), files.len());
    let copied = copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(copied.len(), files.len());
    for rel in files {
        assert_eq!(read(to.path(), rel), rel);
    }
    remove_moved_originals(from.path(), &plan, &copied);
    assert!(!from.path().join("system").exists());
}

#[test]
fn keep_existing_leaves_the_conflicting_original_in_place() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    write(from.path(), "config/global.toml", "old config");
    write(from.path(), "config/system/snes.toml", "old snes");
    write(to.path(), "config/global.toml", "new config");

    let plan = plan_move(from.path(), to.path()).unwrap();
    let copied = copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(copied, [PathBuf::from("config/system/snes.toml")]);
    assert_eq!(read(to.path(), "config/global.toml"), "new config");

    remove_moved_originals(from.path(), &plan, &copied);
    assert_eq!(read(from.path(), "config/global.toml"), "old config");
    assert!(!from.path().join("config/system").exists());
}

#[test]
fn a_file_gone_since_the_plan_is_skipped() {
    // Closing the DB deletes `-wal`; a planned-but-gone file must not stop the move.
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    write(from.path(), "nostyra.db", "db");
    write(from.path(), "nostyra.db-wal", "wal");
    let plan = plan_move(from.path(), to.path()).unwrap();
    fs::remove_file(from.path().join("nostyra.db-wal")).unwrap();

    let copied = copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(copied, [PathBuf::from("nostyra.db")]);
    assert!(!to.path().join("nostyra.db-wal").exists());
}

#[test]
fn nested_folders_overlap() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("data").join("inner");
    fs::create_dir_all(&inner).unwrap();
    let other = root.path().join("other");
    fs::create_dir_all(&other).unwrap();
    assert!(folders_overlap(&root.path().join("data"), &inner));
    assert!(folders_overlap(&inner, &root.path().join("data")));
    assert!(folders_overlap(&other, &other));
    assert!(!folders_overlap(&other, &inner));
}

#[test]
fn web_files_move_and_browser_swap_files_are_dropped() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    let game = "saves/snes/Zelda [AB]";
    for name in [
        "save.srm",
        "save.web.srm",
        "web-slot1.state",
        "web-auto.json",
    ] {
        write(from.path(), &format!("{game}/{name}"), name);
    }
    write(from.path(), &format!("{game}/save.srm.crswap"), "half");
    write(from.path(), "sessions/pending/1700-web.json", "{}");
    write(from.path(), "sessions/pending/1701-web.json.crswap", "half");

    let plan = plan_move(from.path(), to.path()).unwrap();
    assert_eq!((plan.files.len(), plan.swap_files.len()), (5, 2));
    let copied = copy_planned_files(from.path(), to.path(), &plan, false, |_, _| {}).unwrap();
    assert_eq!(
        read(to.path(), &format!("{game}/web-slot1.state")),
        "web-slot1.state"
    );
    assert_eq!(
        read(to.path(), &format!("{game}/save.web.srm")),
        "save.web.srm"
    );
    assert!(!to.path().join(format!("{game}/save.srm.crswap")).exists());

    remove_moved_originals(from.path(), &plan, &copied);
    assert!(
        !from.path().join("saves").exists() && !from.path().join("sessions").exists(),
        "swap files do not keep the old folders alive"
    );
}
