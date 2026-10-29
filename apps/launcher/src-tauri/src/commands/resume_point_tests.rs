//! Resume point reading and warnings against scratch save folders.

use super::*;

/// Lenient, untagged rule with the installed core `version`.
fn lenient(version: Option<&str>) -> StateLoadRule {
    StateLoadRule {
        installed_version: version.map(str::to_owned),
        ..StateLoadRule::default()
    }
}

fn scratch(name: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-resume-{name}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

fn sidecar(sram: Option<u32>, patch: Option<u32>) -> StateSidecar {
    StateSidecar {
        core_name: "Snes9x".into(),
        core_version: "1.63".into(),
        created_at: 1_700_000_000,
        sram_crc32: sram,
        rom_patch_crc32: patch,
        ..StateSidecar::default()
    }
}

fn write_auto(dir: &Path, sidecar: &StateSidecar) {
    std::fs::write(dir.join("auto.state"), [0u8; 16]).unwrap();
    std::fs::write(dir.join("auto.json"), serde_json::to_vec(sidecar).unwrap()).unwrap();
}

#[test]
fn none_without_state_or_valid_sidecar() {
    let temp = scratch("none");
    let dir = temp.path();
    assert_eq!(read_resume_point(dir, None, &lenient(None)), None);
    std::fs::write(dir.join("auto.state"), b"x").unwrap();
    assert_eq!(
        read_resume_point(dir, None, &lenient(None)),
        None,
        "no sidecar"
    );
    std::fs::write(dir.join("auto.json"), b"{not json").unwrap();
    assert_eq!(
        read_resume_point(dir, None, &lenient(None)),
        None,
        "bad sidecar"
    );
    // Only slot files: no resume point.
    let other_temp = scratch("slots-only");
    let other = other_temp.path();
    std::fs::write(other.join("slot0.state"), b"x").unwrap();
    assert_eq!(read_resume_point(other, None, &lenient(None)), None);
}

#[test]
fn valid_point_without_warnings() {
    let temp = scratch("valid");
    let dir = temp.path();
    let sram = b"sram bytes";
    write_auto(dir, &sidecar(Some(crc32fast::hash(sram)), Some(7)));
    std::fs::write(dir.join(SRAM_FILE_NAME), sram).unwrap();
    std::fs::write(dir.join("auto.png"), b"png").unwrap();
    let point = read_resume_point(dir, Some(7), &lenient(Some("1.63"))).unwrap();
    assert_eq!(point.created_at, 1_700_000_000);
    assert_eq!(
        point.thumbnail_path,
        Some(dir.join("auto.png").to_string_lossy().into_owned())
    );
    assert!(point.warnings.is_empty(), "{:?}", point.warnings);
}

#[test]
fn newer_sram_warns_but_missing_srm_does_not() {
    let temp = scratch("sram");
    let dir = temp.path();
    write_auto(dir, &sidecar(Some(crc32fast::hash(b"old")), None));
    let point = read_resume_point(dir, None, &lenient(None)).unwrap();
    assert!(point.warnings.is_empty(), "no .srm yet");
    assert_eq!(point.thumbnail_path, None);
    std::fs::write(dir.join(SRAM_FILE_NAME), b"newer").unwrap();
    let point = read_resume_point(dir, None, &lenient(None)).unwrap();
    assert_eq!(point.warnings, [ResumeWarning::SramNewer]);
}

#[test]
fn patch_and_core_version_changes() {
    let saved = sidecar(None, Some(1));
    assert_eq!(
        resume_warnings(&saved, None, Some(1), &lenient(Some("1.63"))),
        []
    );
    assert_eq!(
        resume_warnings(&saved, None, Some(2), &lenient(Some("1.63"))),
        [ResumeWarning::PatchChanged]
    );
    assert_eq!(
        resume_warnings(&saved, None, None, &lenient(Some("1.64"))),
        [
            ResumeWarning::PatchChanged,
            ResumeWarning::CoreVersionChanged
        ]
    );
    // Unknown version on either side → no warning.
    assert_eq!(resume_warnings(&saved, None, Some(1), &lenient(None)), []);
    let unknown = StateSidecar {
        core_version: String::new(),
        ..saved
    };
    assert_eq!(
        resume_warnings(&unknown, None, Some(1), &lenient(Some("1.64"))),
        []
    );
}

#[test]
fn m3_sidecar_without_new_fields_has_no_warnings() {
    let old = StateSidecar {
        sram_crc32: None,
        rom_patch_crc32: None,
        ..sidecar(None, None)
    };
    assert_eq!(
        resume_warnings(&old, Some(5), None, &lenient(Some("1.63"))),
        []
    );
    let json = serde_json::to_string(&ResumeWarning::CoreVersionChanged).unwrap();
    assert_eq!(json, "\"core-version-changed\"");
}

#[test]
fn patch_crc_follows_the_file_next_to_the_rom() {
    let temp = scratch("patch");
    let dir = temp.path();
    let rom = dir.join("Game.sfc");
    assert_eq!(current_patch_crc32(&rom), None);
    std::fs::write(dir.join("Game.ips"), b"PATCH").unwrap();
    assert_eq!(current_patch_crc32(&rom), Some(crc32fast::hash(b"PATCH")));
    // A second patch in the chain changes the CRC (chain = files concatenated).
    std::fs::write(dir.join("Game.ips1"), b"MORE").unwrap();
    assert_eq!(
        current_patch_crc32(&rom),
        Some(crc32fast::hash(b"PATCHMORE"))
    );
}

#[test]
fn strict_core_flags_another_version_as_incompatible_not_changed() {
    let saved = sidecar(None, None); // core version 1.63
    let strict = |version: &str| StateLoadRule {
        strict_version: true,
        core_display_name: "Dolphin",
        ..lenient(Some(version))
    };
    assert_eq!(
        resume_warnings(&saved, None, None, &strict("1.64")),
        [ResumeWarning::StateIncompatible]
    );
    assert_eq!(resume_warnings(&saved, None, None, &strict("1.63")), []);
    let json = serde_json::to_string(&ResumeWarning::StateIncompatible).unwrap();
    assert_eq!(json, "\"state-incompatible\"");
}

#[test]
fn other_disc_resume_point_is_incompatible_with_its_reason() {
    let temp = scratch("disc");
    let dir = temp.path();
    let disc2 = StateSidecar {
        content_tag: Some("GALE01:1:0".into()),
        ..sidecar(None, None)
    };
    write_auto(dir, &disc2);
    let disc1 = StateLoadRule {
        content_tag: Some("GALE01:0:0".into()),
        ..lenient(Some("1.63"))
    };
    let point = read_resume_point(dir, None, &disc1).unwrap();
    assert_eq!(point.warnings, [ResumeWarning::StateIncompatible]);
    assert_eq!(
        point.incompatible_reason.as_deref(),
        Some("Made from another disc or revision")
    );
    let same = read_resume_point(dir, None, &lenient(Some("1.63"))).unwrap();
    assert_eq!((same.warnings.len(), same.incompatible_reason), (0, None));
}
