use super::*;

pub(crate) fn sample() -> SessionFingerprint {
    SessionFingerprint {
        core_id: "snes9x".into(),
        core_version: "1.62".into(),
        core_sha256: "aa".repeat(32),
        content_sha1: "bb".repeat(20),
        content_crc32: Some(0x1234_5678),
        patch_crc32: None,
        bios_md5s: vec![],
        options_hash: 7,
        system: "snes".into(),
        fps_milli: 60_099,
        platform: "windows-x86_64".into(),
        portable_states: true,
        analog: false,
        warmup_frames: 0,
        max_players: 2,
    }
}

fn only(a: &SessionFingerprint, b: &SessionFingerprint) -> FingerprintMismatch {
    let diff = a.diff(b);
    assert_eq!(diff.len(), 1, "{diff:?}");
    diff.into_iter().next().unwrap_or_else(|| unreachable!())
}

#[test]
fn equal_fingerprints_have_no_diff() {
    assert!(sample().diff(&sample()).is_empty());
}

#[test]
fn each_field_has_a_readable_text() {
    let base = sample();
    let cases: Vec<(SessionFingerprint, &str)> = vec![
        (
            SessionFingerprint {
                core_id: "mesen".into(),
                ..sample()
            },
            "Different core: snes9x vs mesen",
        ),
        (
            SessionFingerprint {
                core_version: "1.63".into(),
                ..sample()
            },
            "Different core version: 1.62 vs 1.63",
        ),
        (
            SessionFingerprint {
                core_sha256: "cc".repeat(32),
                ..sample()
            },
            "Different core build: aaaaaaaaaaaa vs cccccccccccc",
        ),
        (
            SessionFingerprint {
                content_sha1: "dd".repeat(20),
                ..sample()
            },
            "Different game file: bbbbbbbbbbbb vs dddddddddddd",
        ),
        (
            SessionFingerprint {
                content_crc32: None,
                ..sample()
            },
            "Different game data: 12345678 vs none",
        ),
        (
            SessionFingerprint {
                patch_crc32: Some(1),
                ..sample()
            },
            "Different patch: none vs 00000001",
        ),
        (
            SessionFingerprint {
                options_hash: 8,
                ..sample()
            },
            "Different core options: 0000000000000007 vs 0000000000000008",
        ),
        (
            SessionFingerprint {
                system: "nes".into(),
                ..sample()
            },
            "Different system: snes vs nes",
        ),
        (
            SessionFingerprint {
                fps_milli: 50_007,
                ..sample()
            },
            "Different region / frame rate: 60.099 fps vs 50.007 fps",
        ),
        (
            SessionFingerprint {
                platform: "linux-x86_64".into(),
                portable_states: false,
                ..sample()
            },
            "Different platform: windows-x86_64 vs linux-x86_64",
        ),
        (
            SessionFingerprint {
                analog: true,
                ..sample()
            },
            "Different analog setting: off vs on",
        ),
        (
            SessionFingerprint {
                warmup_frames: 3,
                ..sample()
            },
            "Different start-up frames: 0 vs 3",
        ),
        (
            SessionFingerprint {
                max_players: 4,
                ..sample()
            },
            "Different room size: 2 vs 4",
        ),
    ];
    for (other, text) in cases {
        assert_eq!(only(&base, &other).text(), text);
    }
}

#[test]
fn bios_hle_versus_file_is_reported() {
    let hle = SessionFingerprint {
        bios_md5s: vec!["hle".into()],
        ..sample()
    };
    let file = SessionFingerprint {
        bios_md5s: vec!["924e392ed05558ffdb115408c263dccf".into()],
        ..sample()
    };
    let mismatch = only(&hle, &file);
    assert_eq!(mismatch.field, FingerprintField::Bios);
    assert_eq!(mismatch.text(), "Different BIOS: hle vs 924e392ed055");
}

#[test]
fn hashes_equal_in_their_short_form_still_differ() {
    let a = SessionFingerprint {
        core_sha256: format!("{}0", "a".repeat(63)),
        ..sample()
    };
    let b = SessionFingerprint {
        core_sha256: format!("{}1", "a".repeat(63)),
        ..sample()
    };
    assert_eq!(only(&a, &b).field, FingerprintField::CoreBuild);
}

#[test]
fn cross_platform_ignores_core_build_when_states_are_portable() {
    let mac = SessionFingerprint {
        platform: "macos-aarch64".into(),
        core_sha256: "cc".repeat(32),
        ..sample()
    };
    assert!(sample().diff(&mac).is_empty());
    let older_version = SessionFingerprint {
        core_version: "1.61".into(),
        ..mac.clone()
    };
    assert_eq!(
        only(&sample(), &older_version).field,
        FingerprintField::CoreVersion
    );
    let not_portable = SessionFingerprint {
        portable_states: false,
        ..mac
    };
    assert_eq!(
        only(&sample(), &not_portable).field,
        FingerprintField::Platform
    );
}

#[test]
fn same_platform_still_requires_the_same_core_build() {
    let other_build = SessionFingerprint {
        core_sha256: "cc".repeat(32),
        ..sample()
    };
    assert_eq!(
        only(&sample(), &other_build).field,
        FingerprintField::CoreBuild
    );
}

#[test]
fn platform_tag_names_this_build() {
    assert!(PLATFORM.contains(std::env::consts::OS), "{PLATFORM}");
    assert!(PLATFORM.ends_with(std::env::consts::ARCH), "{PLATFORM}");
}
