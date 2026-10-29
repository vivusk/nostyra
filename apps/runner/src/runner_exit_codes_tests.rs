//! Exit-code mapping and the ROM check (`runner_exit_codes`).

use super::*;
use anyhow::Context;

#[test]
fn maps_host_and_rom_errors_through_context() {
    let core = Err::<(), _>(HostError::MissingSymbol("retro_run")).context("starting");
    assert_eq!(exit_code_for(&core.unwrap_err()), EXIT_CORE_LOAD);
    let api = Err::<(), _>(HostError::ApiVersion(2)).context("starting");
    assert_eq!(exit_code_for(&api.unwrap_err()), EXIT_CORE_LOAD);
    let rom = Err::<(), _>(HostError::LoadGameRejected(None)).context("starting");
    assert_eq!(exit_code_for(&rom.unwrap_err()), EXIT_ROM_LOAD);
    let staged = Err::<(), _>(HostError::StageContent {
        path: "content.md".into(),
        source: std::io::ErrorKind::InvalidInput.into(),
    })
    .context("starting");
    assert_eq!(exit_code_for(&staged.unwrap_err()), EXIT_ROM_LOAD);
    let too_big = RomValidationError::TooLarge {
        path: "x".into(),
        size: 1,
        max: 0,
    };
    assert_eq!(exit_code_for(&too_big.into()), EXIT_ROM_LOAD);
    let unclassified = RomValidationError::Unclassified {
        path: "x.bin".into(),
        reason: "no candidate".into(),
    };
    assert_eq!(exit_code_for(&unclassified.into()), EXIT_ROM_LOAD);
    assert_eq!(exit_code_for(&anyhow::anyhow!("gpu lost")), EXIT_RUNTIME);
    let no_gl = HostError::HwRenderUnavailable("no OpenGL".into());
    assert_eq!(exit_code_for(&no_gl.into()), EXIT_GRAPHICS);
}

#[test]
fn missing_rom_is_rejected() {
    assert!(matches!(
        validate_rom(Path::new("no/such.sfc"), None),
        Err(RomValidationError::NotAFile(_))
    ));
    assert!(validate_rom(&std::env::temp_dir(), None).is_err()); // a directory
}

#[test]
fn system_comes_from_the_flag_or_the_content() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-validate-")
        .tempdir()
        .unwrap();
    let mut nes = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    nes.resize(16 + 24 * 1024, 0);
    let headered = temp.path().join("game.nes");
    std::fs::write(&headered, &nes).unwrap();
    assert_eq!(validate_rom(&headered, None).unwrap().id, "nes");
    assert_eq!(validate_rom(&headered, Some("nes")).unwrap().id, "nes");
    // Size limits come from the system: 4 KiB is below the NES floor.
    let tiny = temp.path().join("tiny.nes");
    std::fs::write(&tiny, &nes[..4096]).unwrap();
    assert!(matches!(
        validate_rom(&tiny, Some("nes")),
        Err(RomValidationError::TooSmall { min, .. }) if min == system_catalog::NES.min_bytes
    ));
    let bare = temp.path().join("bare.nes");
    std::fs::write(&bare, &nes[16..]).unwrap();
    assert!(matches!(
        validate_rom(&bare, None),
        Err(RomValidationError::Unclassified { .. })
    ));
    assert!(matches!(
        validate_rom(&headered, Some("no-such-system")),
        Err(RomValidationError::Unclassified { .. })
    ));
}

#[test]
fn broken_disc_fails_with_the_scanner_reason() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-validate-disc-")
        .tempdir()
        .unwrap();
    let cue = temp.path().join("Game.cue");
    std::fs::write(
        &cue,
        "FILE \"Game_D13.bin\" BINARY
  TRACK 01 MODE2/2352
",
    )
    .unwrap();
    let reason = "track file 'Game_D13.bin' not found — fix the FILE line or rename the file";
    let flagged = validate_rom(&cue, Some(system_catalog::PSX.id)).unwrap_err();
    assert!(matches!(&flagged, RomValidationError::BrokenDisc { reason: r, .. } if r == reason));
    assert_eq!(exit_code_for(&flagged.into()), EXIT_ROM_LOAD);
    assert!(matches!(
        validate_rom(&cue, None),
        Err(RomValidationError::Unclassified { reason: r, .. }) if r == reason
    ));
}

#[test]
fn unplayable_ds_dumps_exit_4_with_the_reason() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-validate-nds-")
        .tempdir()
        .unwrap();
    let nds = system_catalog::NDS.id;
    // Retail layout (ARM9 @0x4000) without the decrypted marker → encrypted.
    let mut rom = vec![0u8; 0x5000];
    rom[0x0C..0x10].copy_from_slice(b"ZNSB");
    rom[0x20..0x24].copy_from_slice(&0x4000u32.to_le_bytes());
    let encrypted = temp.path().join("encrypted.nds");
    std::fs::write(&encrypted, &rom).unwrap();
    let refused = validate_rom(&encrypted, Some(nds)).unwrap_err();
    assert!(refused.to_string().starts_with(
        "This DS dump is encrypted; Nostyra plays decrypted dumps (No-Intro Decrypted): "
    ));
    assert_eq!(exit_code_for(&refused.into()), EXIT_ROM_LOAD);
    rom[0x4000..0x4004].copy_from_slice(&0xE7FF_DEFFu32.to_le_bytes());
    let decrypted = temp.path().join("decrypted.nds");
    std::fs::write(&decrypted, &rom).unwrap();
    assert_eq!(validate_rom(&decrypted, Some(nds)).unwrap().id, nds);
    rom[0x12] = 3;
    let dsi = temp.path().join("dsi.nds");
    std::fs::write(&dsi, &rom).unwrap();
    let refused = validate_rom(&dsi, Some(nds)).unwrap_err();
    assert!(refused
        .to_string()
        .starts_with(system_catalog::NDS_DSI_ONLY));
}

#[test]
fn core_load_rejection_keeps_exit_4_and_shows_the_cores_reason() {
    let reason = "ROM isn't valid, did you select the right file?";
    let rejected = Err::<(), _>(HostError::LoadGameRejected(Some(reason.into())))
        .context("cannot start x.nds with melondsds_libretro.dll")
        .unwrap_err();
    assert_eq!(exit_code_for(&rejected), EXIT_ROM_LOAD);
    assert!(format!("{rejected:#}").ends_with(&format!("returned false): {reason}")));
}
