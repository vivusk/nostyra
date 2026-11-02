//! ZIP playback keeps game identity while the core reads temporary ROM content.
#![allow(clippy::unwrap_used)]

mod support;

use std::io::Write;
use std::path::Path;
use support::{core, result_line, runner, scratch, test_rom};
use zip::{write::SimpleFileOptions, ZipWriter};

fn archive(path: &Path, bytes: &[u8]) {
    let mut zip = ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file("nested/Inner Game.SFC", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap();
}

fn assert_no_staged_roms(data: &Path) {
    fn files(path: &Path) -> usize {
        std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| {
                if entry.path().is_dir() {
                    files(&entry.path())
                } else {
                    1
                }
            })
            .sum()
    }
    assert_eq!(files(&ipc_protocol::staged_content_dir(data)), 0);
}

#[test]
fn zip_plays_same_frames_keeps_saves_config_and_session_identity_and_cleans_up() {
    let temp = scratch("zip-play");
    let rom = temp.join("Archive Game.zip");
    let bytes = std::fs::read(test_rom()).unwrap();
    archive(&rom, &bytes);
    let original = std::fs::read(&rom).unwrap();
    let data = temp.join("data");
    let plain = runner(
        &core(),
        &test_rom(),
        &temp.join("plain"),
        &["--headless-frames", "120"],
    );
    let zipped = runner(&core(), &rom, &data, &["--headless-frames", "120", "--ipc"]);
    assert_eq!(
        zipped.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&zipped.stderr)
    );
    assert_eq!(result_line(&plain), result_line(&zipped));
    let save_dir = support::game_saves_dir(&data, "snes", &rom);
    assert!(save_dir
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("Archive Game ["));
    assert!(save_dir.join("save.srm").is_file());
    let pending = std::fs::read_dir(ipc_protocol::pending_sessions_dir(&data))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let record: ipc_protocol::PendingSessionRecord =
        serde_json::from_slice(&std::fs::read(pending).unwrap()).unwrap();
    assert_eq!(record.rom_path, rom.to_str().unwrap());
    let config_path = support::game_config_path(&data, "snes", &rom);
    config_layers::set_layer_value(
        &config_path,
        "video.scaling",
        Some(config_layers::Value::String("integer".into())),
    )
    .unwrap();
    let config = runner(&core(), &rom, &data, &["--print-config"]);
    assert_eq!(
        config.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&config.stderr)
    );
    // Cores can flush their own stdout after the config JSON (same contract as
    // runner_print_config_tests); consume one JSON value rather than the stream.
    let stdout = String::from_utf8_lossy(&config.stdout);
    let json = &stdout[stdout.find('{').unwrap()..];
    let config: serde_json::Value = serde_json::Deserializer::from_str(json)
        .into_iter()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(config["video"]["scaling"], "integer");
    assert_eq!(std::fs::read(&rom).unwrap(), original);
    assert_no_staged_roms(&data);
}

#[test]
fn zip_rejects_wrong_system_before_loading_core_and_cleans_up_after_core_failure() {
    let temp = scratch("zip-error");
    let rom = temp.join("game.zip");
    archive(&rom, &std::fs::read(test_rom()).unwrap());
    let data = temp.join("data");
    let missing_core = temp.join("no-such-core");
    let wrong = runner(
        &missing_core,
        &rom,
        &data,
        &["--system", "nes", "--headless-frames", "1"],
    );
    assert_eq!(
        wrong.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&wrong.stderr)
    );
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("system"));
    let failed = runner(&missing_core, &rom, &data, &["--headless-frames", "1"]);
    assert_eq!(
        failed.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    assert_no_staged_roms(&data);
    std::fs::write(&rom, b"corrupt zip").unwrap();
    let corrupt = runner(
        &missing_core,
        &rom,
        &data,
        &["--system", "snes", "--headless-frames", "1"],
    );
    assert_eq!(corrupt.status.code(), Some(4));
}

#[test]
fn zip_uses_patches_next_to_archive_and_records_their_crc() {
    let temp = scratch("zip-patch");
    let rom = temp.join("game.zip");
    archive(&rom, &std::fs::read(test_rom()).unwrap());
    let clean = runner(
        &core(),
        &rom,
        &temp.join("clean"),
        &["--headless-frames", "120"],
    );
    assert_eq!(clean.status.code(), Some(0));
    let mut patch = b"PATCH".to_vec();
    for offset in (0..0x7000u32).step_by(0x1000) {
        patch.extend_from_slice(&offset.to_be_bytes()[1..]);
        patch.extend_from_slice(&[0x10, 0x00]);
        patch.extend(std::iter::repeat_n(0, 0x1000));
    }
    patch.extend_from_slice(b"EOF");
    std::fs::write(temp.join("game.ips"), &patch).unwrap();
    let data = temp.join("patched");
    let patched = runner(
        &core(),
        &rom,
        &data,
        &["--headless-frames", "120", "--headless-save-slot", "auto"],
    );
    assert_eq!(
        patched.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&patched.stderr)
    );
    assert_ne!(result_line(&clean), result_line(&patched));
    let sidecar = support::game_saves_dir(&data, "snes", &rom).join("auto.json");
    let sidecar: ipc_protocol::StateSidecar =
        serde_json::from_slice(&std::fs::read(sidecar).unwrap()).unwrap();
    assert!(sidecar.rom_patch_crc32.is_some());
    assert_no_staged_roms(&data);
}

#[test]
fn allowed_cartridge_cores_play_zipped_and_plain_roms_identically() {
    let temp = scratch("zip-cartridge-cores");
    for (ext, core_id, fetch) in [
        ("nes", "mesen", "fetch-mesen-core.sh"),
        ("gba", "mgba", "fetch-mgba-core.sh"),
        ("gen", "genesis_plus_gx", "fetch-genesis-plus-gx-core.sh"),
        ("sms", "genesis_plus_gx", "fetch-genesis-plus-gx-core.sh"),
        ("gg", "genesis_plus_gx", "fetch-genesis-plus-gx-core.sh"),
        ("sg", "genesis_plus_gx", "fetch-genesis-plus-gx-core.sh"),
    ] {
        let Some(core) = support::registry_core(core_id, fetch) else {
            continue;
        };
        let plain = support::repo(&format!("test-assets/roms/nostyra-test.{ext}"));
        let rom = temp.join(format!("Game-{ext}.zip"));
        let mut zip = ZipWriter::new(std::fs::File::create(&rom).unwrap());
        zip.start_file(
            format!("nested/Game.{}", ext.to_ascii_uppercase()),
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&std::fs::read(&plain).unwrap()).unwrap();
        zip.finish().unwrap();
        let data = temp.join(format!("zip-{ext}"));
        let zipped = runner(&core, &rom, &data, &["--headless-frames", "120"]);
        assert_eq!(
            zipped.status.code(),
            Some(0),
            "{ext}: {}",
            String::from_utf8_lossy(&zipped.stderr)
        );
        let normal = runner(
            &core,
            &plain,
            &temp.join(format!("plain-{ext}")),
            &["--headless-frames", "120"],
        );
        assert_eq!(
            normal.status.code(),
            Some(0),
            "{ext}: {}",
            String::from_utf8_lossy(&normal.stderr)
        );
        assert_eq!(result_line(&zipped), result_line(&normal), "{ext}");
        assert_no_staged_roms(&data);
    }
}
