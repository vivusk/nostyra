//! Master System, Game Gear and SG-1000 end to end through the runner binary on
//! Genesis Plus GX with the self-written test ROMs (`--system sms|gg|sg1000`):
//! SMS / GG cart RAM persists (boot counter) with a stable 2-byte `.srm` and no
//! `.srm.bak`; the SG-1000 cart has no battery RAM, so no `save.srm` is written; a
//! slot saved headless loads before frame 1 on all three.
//!
//! Skips with a note when the core is missing (`scripts/fetch-genesis-plus-gx-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{repo, result_line, runner, scratch};

/// System → test ROM extension.
const SYSTEMS: [(&str, &str); 3] = [("sms", "sms"), ("gg", "gg"), ("sg1000", "sg")];

fn gpgx() -> Option<PathBuf> {
    support::registry_core("genesis_plus_gx", "fetch-genesis-plus-gx-core")
}

fn rom(ext: &str) -> PathBuf {
    repo(&format!("test-assets/roms/nostyra-test.{ext}"))
}

fn run_ok(core: &Path, data: &Path, system: &str, ext: &str, extra: &[&str]) -> String {
    let mut args = vec!["--system", system];
    args.extend_from_slice(extra);
    let out = runner(core, &rom(ext), data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{system}: {stderr}");
    result_line(&out)
}

#[test]
fn eight_bit_cart_ram_persists_only_where_the_cart_has_it() {
    let Some(core) = gpgx() else { return };
    for (system, ext) in SYSTEMS {
        let data = scratch(&format!("sega-8bit-sram-{system}"));
        let dir = support::game_saves_dir(&data, system, &rom(ext));
        let srm = dir.join(ipc_protocol::SRAM_FILE_NAME);
        let frames = ["--headless-frames", "30"];
        run_ok(&core, &data, system, ext, &frames);
        if system == "sg1000" {
            run_ok(&core, &data, system, ext, &frames);
            assert!(!srm.exists(), "sg1000: untouched RAM → no save.srm");
            continue;
        }
        assert_eq!(
            std::fs::read(&srm).unwrap(),
            [1, 0xA5],
            "{system}: counter 1"
        );
        run_ok(&core, &data, system, ext, &frames);
        assert_eq!(
            std::fs::read(&srm).unwrap(),
            [2, 0xA5],
            "{system}: counter 2"
        );
        assert!(!dir.join("save.srm.bak").exists(), "{system}: no .srm.bak");
    }
}

#[test]
fn eight_bit_saved_slot_loads_before_first_frame() {
    let Some(core) = gpgx() else { return };
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    for (system, ext) in SYSTEMS {
        let data = scratch(&format!("sega-8bit-slot-{system}"));
        let save = ["--headless-frames", "90", "--headless-save-slot", "1"];
        let saved = run_ok(&core, &data, system, ext, &save);
        assert!(saved.ends_with("loaded=false"), "{system}: {saved}");
        let load = ["--headless-frames", "5", "--load-state", "1"];
        let warm1 = run_ok(&core, &data, system, ext, &load);
        let warm2 = run_ok(&core, &data, system, ext, &load);
        assert!(warm1.ends_with("loaded=true"), "{system}: {warm1}");
        assert_eq!(checksum(&warm1), checksum(&warm2), "{system}: same frames");
        let cold = run_ok(&core, &data, system, ext, &["--headless-frames", "5"]);
        assert_ne!(checksum(&cold), checksum(&warm1), "{system}: slot loaded");
    }
}
