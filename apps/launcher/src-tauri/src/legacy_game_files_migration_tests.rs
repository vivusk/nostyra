use super::*;

const A: &str = "aaaaaaaa11111111111111111111111111111111";
const B: &str = "bbbbbbbb22222222222222222222222222222222";

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("nostyra-legacy-files-")
        .tempdir()
        .unwrap()
}

fn key(id: i64, system: &str, rom: &str, sha1: &str) -> GameFilesKey {
    GameFilesKey {
        game_id: id,
        platform_id: system.into(),
        rom_path: rom.into(),
        sha1: sha1.into(),
    }
}

/// `saves/<system>/<name>/save.srm` + `config/game/<system>/<name>.toml`.
fn write_legacy(data: &Path, system: &str, name: &str) {
    let saves = data.join("saves").join(system).join(name);
    std::fs::create_dir_all(saves.join("sub")).unwrap();
    std::fs::write(saves.join("save.srm"), b"sram").unwrap();
    std::fs::write(saves.join("sub").join("card2.mcd"), b"card").unwrap();
    let config = data.join("config/game").join(system);
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join(format!("{name}.toml")), b"[video]").unwrap();
}

fn has_files(data: &Path, system: &str, name: &str) -> bool {
    let saves = data.join("saves").join(system).join(name);
    saves.join("save.srm").is_file()
        && saves.join("sub").join("card2.mcd").is_file()
        && data
            .join("config/game")
            .join(system)
            .join(format!("{name}.toml"))
            .is_file()
}

#[test]
fn single_claim_is_renamed_with_its_id() {
    let temp = scratch();
    let data = temp.path();
    write_legacy(data, "psx", "Game (USA)");
    // Missing rows count too (their saves are still theirs); the other key is unrelated.
    let keys = [
        key(1, "psx", "E:/PSX/Game (USA).m3u", A),
        key(2, "snes", "E:/SNES/Game (USA).sfc", B),
    ];
    let summary = migrate_legacy_game_files(data, &keys);
    assert_eq!((summary.renamed, summary.copied), (2, 0), "{summary:?}");
    assert!(summary.kept.is_empty());
    assert!(has_files(data, "psx", "Game (USA) [AAAAAAAA]"));
    assert!(!data.join("saves/psx/Game (USA)").exists());
    // Second start: nothing left to do.
    assert_eq!(
        migrate_legacy_game_files(data, &keys),
        LegacyMigrationSummary::default()
    );
}

#[test]
fn shared_file_name_is_copied_to_each_game() {
    let temp = scratch();
    let data = temp.path();
    write_legacy(data, "snes", "Game");
    let keys = [
        key(1, "snes", "E:/A/Game.sfc", A),
        key(2, "snes", "E:/B/Game.smc", B),
        // A duplicate copy of A: same id, no extra claim.
        key(3, "snes", "E:/C/Game.sfc", A),
    ];
    let summary = migrate_legacy_game_files(data, &keys);
    assert_eq!((summary.renamed, summary.copied), (0, 4), "{summary:?}");
    assert_eq!(summary.kept.len(), 2);
    assert!(has_files(data, "snes", "Game [AAAAAAAA]"));
    assert!(has_files(data, "snes", "Game [BBBBBBBB]"));
    assert!(has_files(data, "snes", "Game"), "legacy stays");
}

#[test]
fn existing_id_entry_or_bad_sha1_keeps_legacy() {
    let temp = scratch();
    let data = temp.path();
    write_legacy(data, "nes", "Game");
    write_legacy(data, "nes", "Renamed later [aaaaaaaa]");
    write_legacy(data, "nes", "NoHash");
    let keys = [
        key(1, "nes", "E:/NES/Game.nes", A),
        key(2, "nes", "E:/NES/NoHash.nes", ""),
    ];
    let summary = migrate_legacy_game_files(data, &keys);
    assert_eq!((summary.renamed, summary.copied), (0, 0));
    assert_eq!(summary.kept.len(), 2, "{summary:?}");
    assert!(has_files(data, "nes", "Game") && has_files(data, "nes", "NoHash"));
}
