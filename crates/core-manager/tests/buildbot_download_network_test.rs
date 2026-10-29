//! Real download from the libretro buildbot. Ignored by default (needs network);
//! run manually or in a nightly job: `cargo test -p core-manager -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::atomic::AtomicBool;

use core_manager::{install_core, installed_core, CoreInfo};

#[test]
#[ignore = "downloads snes9x from buildbot.libretro.com"]
fn downloads_and_installs_snes9x_with_stub_probe() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-core-net-")
        .tempdir()
        .unwrap();
    let dir = temp.path();
    let mut last_progress = (0, None);

    let core = install_core(
        "snes9x",
        dir,
        |path| {
            let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
            if size < 100_000 {
                return Err(format!("suspiciously small core: {size} bytes"));
            }
            Ok(CoreInfo {
                name: "stub".into(),
                version: "stub".into(),
            })
        },
        |done, total| last_progress = (done, total),
        &AtomicBool::new(false),
    )
    .expect("buildbot download");

    assert_eq!(installed_core(dir, "snes9x"), Some(core.path.clone()));
    assert_eq!(core.sha256.len(), 64);
    assert!(last_progress.0 > 0);
    if let Some(total) = last_progress.1 {
        assert_eq!(last_progress.0, total);
    }
}
