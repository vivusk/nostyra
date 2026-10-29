//! Buildbot URL resolution per registry core and target.

#![allow(clippy::unwrap_used)] // test code: panics are the failure signal
use super::*;

fn url(os: &str, arch: &str) -> String {
    buildbot_core_source("snes9x", os, arch)
        .unwrap()
        .archive_url
}

#[test]
fn resolves_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    assert_eq!(
        url("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/snes9x_libretro.dll.zip")
    );
    assert_eq!(
        url("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/snes9x_libretro.so.zip")
    );
    assert_eq!(
        url("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/snes9x_libretro.dylib.zip")
    );
    assert_eq!(
        url("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/snes9x_libretro.dylib.zip")
    );
}

#[test]
fn mesen_resolves_on_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    let mesen = |os, arch| buildbot_core_source("mesen", os, arch).unwrap().archive_url;
    assert_eq!(
        mesen("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/mesen_libretro.dll.zip")
    );
    assert_eq!(
        mesen("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/mesen_libretro.so.zip")
    );
    assert_eq!(
        mesen("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/mesen_libretro.dylib.zip")
    );
    assert_eq!(
        mesen("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/mesen_libretro.dylib.zip")
    );
}

#[test]
fn mgba_resolves_on_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    let mgba = |os, arch| buildbot_core_source("mgba", os, arch).unwrap().archive_url;
    assert_eq!(
        mgba("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/mgba_libretro.dll.zip")
    );
    assert_eq!(
        mgba("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/mgba_libretro.so.zip")
    );
    assert_eq!(
        mgba("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/mgba_libretro.dylib.zip")
    );
    assert_eq!(
        mgba("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/mgba_libretro.dylib.zip")
    );
}

#[test]
fn genesis_plus_gx_resolves_on_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    let gpgx = |os, arch| {
        buildbot_core_source("genesis_plus_gx", os, arch)
            .unwrap()
            .archive_url
    };
    assert_eq!(
        gpgx("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/genesis_plus_gx_libretro.dll.zip")
    );
    assert_eq!(
        gpgx("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/genesis_plus_gx_libretro.so.zip")
    );
    assert_eq!(
        gpgx("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/genesis_plus_gx_libretro.dylib.zip")
    );
    assert_eq!(
        gpgx("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/genesis_plus_gx_libretro.dylib.zip")
    );
}

#[test]
fn mupen64plus_next_resolves_on_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    let mupen = |os, arch| {
        buildbot_core_source("mupen64plus-next", os, arch)
            .unwrap()
            .archive_url
    };
    assert_eq!(
        mupen("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/mupen64plus_next_libretro.dll.zip")
    );
    assert_eq!(
        mupen("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/mupen64plus_next_libretro.so.zip")
    );
    assert_eq!(
        mupen("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/mupen64plus_next_libretro.dylib.zip")
    );
    assert_eq!(
        mupen("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/mupen64plus_next_libretro.dylib.zip")
    );
}

#[test]
fn pcsx_rearmed_resolves_on_every_supported_target() {
    let base = BUILDBOT_BASE_URL;
    let pcsx = |os, arch| {
        buildbot_core_source("pcsx-rearmed", os, arch)
            .unwrap()
            .archive_url
    };
    assert_eq!(
        pcsx("windows", "x86_64"),
        format!("{base}/windows/x86_64/latest/pcsx_rearmed_libretro.dll.zip")
    );
    assert_eq!(
        pcsx("linux", "x86_64"),
        format!("{base}/linux/x86_64/latest/pcsx_rearmed_libretro.so.zip")
    );
    assert_eq!(
        pcsx("macos", "aarch64"),
        format!("{base}/apple/osx/arm64/latest/pcsx_rearmed_libretro.dylib.zip")
    );
    assert_eq!(
        pcsx("macos", "x86_64"),
        format!("{base}/apple/osx/x86_64/latest/pcsx_rearmed_libretro.dylib.zip")
    );
}

#[test]
fn library_name_matches_platform_extension() {
    let source = buildbot_core_source("snes9x", "linux", "x86_64").unwrap();
    assert_eq!(source.library_file_name, "snes9x_libretro.so");
}

#[test]
fn rejects_unknown_core_and_platform() {
    assert!(matches!(
        buildbot_core_source("../evil", "windows", "x86_64"),
        Err(CoreManagerError::UnknownCore(_))
    ));
    assert!(matches!(
        buildbot_core_source("snes9x", "linux", "aarch64"),
        Err(CoreManagerError::UnsupportedPlatform { .. })
    ));
}

#[test]
fn current_platform_resolves_on_supported_hosts() {
    let source = current_platform_core_source("snes9x").unwrap();
    assert!(source.archive_url.starts_with("https://"));
    assert!(source
        .library_file_name
        .ends_with(std::env::consts::DLL_EXTENSION));
}
