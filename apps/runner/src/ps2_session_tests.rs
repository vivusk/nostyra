//! A PlayStation 2 session on a fake PCEE2 (no DLL): the registry's fixed options
//! plus the per-launch `pcsx2_bios` answer, the state rules the session applies
//! (`Serialized`, 128 MiB cap, exact size, no requery, strict version, disc tag),
//! the pad ports, the per-disc content tag on the session paths and the slower
//! periodic resume point. The zero-BIOS exit 4 and the picker live in
//! `ps2_bios_option_tests`; the refused-GL warning in the parent's tests.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::time::{Duration, Instant};

use disc_image::test_images::to_iso;
use disc_image::test_ps2_images::{ps2_cnf, ps2_volume};
use ipc_protocol::{StateSidecar, StateSlotId};
use libretro_host::CoreInfo;
use system_catalog::ps2_bios_test_image::ps2_bios_image;
use system_catalog::{firmware_dir, SaveStateQuality, PCEE2, PS2};

use crate::bios_option_switch::launch_option;
use crate::bios_option_switch::ps2_bios_option::tests::memo_lock;
use crate::emulation_main_loop::save_state_policy::{state_refusal, STATES_NOT_FRAME_EXACT};
use crate::periodic_resume_save::{PeriodicResumeSave, LARGE_STATE_INTERVAL};
use crate::runner_core_options::{self, resolve};
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_session_config::prepare_core;
use crate::state_file_guard::StateFileGuard;
use crate::state_load_checks::check_state_size;

const MIB: u64 = 1024 * 1024;
const SHA1: &str = "fa7546d5ab6d3b0c8f4b5ef0a2c33d4f26b7e6a9";

/// A PS2 `.iso` booting `SLUS_203.12` (NTSC) under `dir`.
fn us_disc(dir: &Path) -> std::path::PathBuf {
    let cnf = ps2_cnf("SLUS_203.12");
    let sectors = ps2_volume(
        "NOSTYRA_PS2",
        &[("SYSTEM.CNF", cnf.as_bytes()), ("SLUS_203.12", &[7; 900])],
    );
    let path = dir.join("game.iso");
    std::fs::write(&path, to_iso(&sectors)).unwrap();
    path
}

#[test]
fn fixed_options_and_the_bios_answer_make_up_the_session_options() {
    let _serial = memo_lock();
    let temp = tempfile::tempdir().unwrap();
    let bios = temp.path().join("pcsx2/bios");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::write(
        bios.join("scph39001.bin"),
        ps2_bios_image("0160AC20020207", 0x2740),
    )
    .unwrap();
    let rom = us_disc(temp.path());
    let core = temp.path().join("cores/pcee2_libretro.dll"); // never opened
    prepare_core(&PS2, &core, temp.path(), &rom).expect("a dump is present");
    let bios = launch_option(&PS2, temp.path(), &rom).expect("answered");
    assert_eq!(bios, ("pcsx2_bios".into(), "scph39001.bin".into()));
    let options = resolve(&PS2, std::slice::from_ref(&bios));
    for (key, value) in PCEE2.fixed_options {
        assert!(options.contains(&((*key).into(), (*value).into())), "{key}");
    }
    assert_eq!(
        options.len(),
        PCEE2.fixed_options.len() + 1,
        "bios appended"
    );
    assert_eq!(options.last(), Some(&bios));
    // A hidden `--core-option` (tests) replaces a fixed value, as for other cores.
    let forced = resolve(&PS2, &[bios, ("pcsx2_renderer".into(), "software".into())]);
    assert!(forced.contains(&("pcsx2_renderer".into(), "software".into())));
    assert!(!forced.contains(&("pcsx2_renderer".into(), "opengl".into())));
}

#[test]
fn state_rules_of_a_fake_pcee2_follow_the_registry() {
    assert_eq!(
        runner_core_options::savestate_quality(&PS2),
        SaveStateQuality::Serialized
    );
    assert_eq!(
        state_refusal(false, SaveStateQuality::Serialized),
        Some(STATES_NOT_FRAME_EXACT),
        "run-ahead and rewind refused"
    );
    assert!(runner_core_options::exact_state_size(&PS2));
    assert!(
        !runner_core_options::state_size_requery(&PS2),
        "no boot hold"
    );
    assert!(
        runner_core_options::keep_library_loaded(&PS2),
        "unload crashes in dll_unloaded code → hard exit like Dolphin"
    );
    assert_eq!(runner_core_options::port_devices(&PS2), [1, 1, 0, 0]);
    let max = runner_core_options::max_state_bytes(&PS2);
    assert_eq!(max, 128 * MIB);
    let slot = StateSlotId::Numbered(1);
    check_state_size(slot, 96 * MIB, max).expect("a PCEE2 state fits");
    assert!(check_state_size(slot, 128 * MIB + 1, max).is_err());
    // Strict version + disc tag, before any byte reaches the core.
    let pcee2 = CoreInfo {
        name: "PCEE2".into(),
        version: "v2.9.138".into(),
    };
    let disc1 = StateFileGuard::new(&PS2, Some("SLUS_203.12;1:1A2B3C4D".into()));
    let mut saved = StateSidecar {
        core_name: "PCEE2".into(),
        core_version: "v2.9.138".into(),
        created_at: 1,
        ..StateSidecar::default()
    };
    disc1.stamp(&mut saved, b"state");
    assert!(disc1.check_sidecar(&saved, &pcee2).is_ok());
    saved.core_version = "v2.9.200".into();
    let text = format!("{:#}", disc1.check_sidecar(&saved, &pcee2).unwrap_err());
    assert_eq!(text, "state was made by another PCEE2 version");
    saved.core_version = "v2.9.138".into();
    StateFileGuard::new(&PS2, Some("SLUS_203.12;1:FFFFFFFF".into())).stamp(&mut saved, b"state");
    let text = format!("{:#}", disc1.check_sidecar(&saved, &pcee2).unwrap_err());
    assert_eq!(text, "state was made from another disc or revision");
    saved.content_tag = None; // a state without a tag is not refused
    assert!(disc1.check_sidecar(&saved, &pcee2).is_ok());
}

#[test]
fn session_paths_tag_states_by_disc_and_keep_bios_under_system() {
    let temp = tempfile::tempdir().unwrap();
    let rom = us_disc(temp.path());
    let root = temp.path().join("data");
    let paths = RunnerDataPaths::resolve(Some(&root), &rom, &PS2, Some(SHA1)).unwrap();
    let tag = paths.state_content_tag.expect("PS2 discs are tagged");
    assert!(tag.starts_with("SLUS_203.12;1:"), "{tag}");
    assert_eq!(paths.system_dir, root.join("system"));
    assert_eq!(
        firmware_dir(&paths.system_dir, &PCEE2),
        root.join("system").join("pcsx2").join("bios"),
        "BIOS, .nvm/.mec and memcards live under system/pcsx2, never in saves/"
    );
    assert!(paths.saves_dir.starts_with(root.join("saves").join("ps2")));
    assert_eq!(paths.states_dir, paths.saves_dir);
}

#[test]
fn ps2_resume_points_are_saved_at_the_large_state_interval() {
    let start = Instant::now();
    let mut ps2 = PeriodicResumeSave::for_system(PS2.id, start);
    let frames = crate::auto_save_on_exit::AUTO_SAVE_MIN_FRAMES;
    assert!(!ps2.due(start + Duration::from_secs(60), frames, true));
    assert!(ps2.due(start + LARGE_STATE_INTERVAL, frames, true));
}
