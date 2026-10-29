//! Dolphin on its Null renderer (no OpenGL, every OS): the GC DOL boots, counts VI
//! fields in MEM1, sees held pad buttons / sticks / analog triggers in its SI words,
//! plays its tone and latches the PI reset button on reset; the Wii DOL boots in Wii mode. No state call
//! may reach the core here (it segfaults under Null): the session's state actions
//! are refused cleanly through the runner's own paths instead.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use ipc_protocol::{StateSidecar, StateSlotId};
use libretro_host::{CoreSession, HostError};
use system_catalog::{GC, WII};
use video_renderer::RgbaFrameBuffer;

use super::checks::{boot, check_audio, check_field_counter, check_reset, pad_words, run};
use super::{debug_word, dolphin, load_dol, options, system_folder, GC_DOL, HID4_AT_BOOT};
use super::{SI_IN_HI, WII_DOL};
use crate::auto_save_on_exit::{skip_reason, ExitFacts};
use crate::game_save_data::GameSaveData;
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};
use crate::save_state_slots::STATES_NEED_HW_CONTEXT;
use crate::startup_state_load::{StartupStateLoad, COULD_NOT_RESUME};

const NULL: (&str, &str) = ("dolphin_renderer", "Null");
/// SI IN_HI with nothing held: no buttons, bit 23 set, both stick axes centred.
const IDLE_HI: u32 = 0x0080_8080;
/// RetroPad A (joypad id 8) = GameCube A.
const RETRO_A: u32 = 8;

#[test]
fn gamecube_wii_null_suite_dolphin() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(core) = dolphin() else { return };
    let system = system_folder(core);
    {
        eprintln!("dolphin null suite: GameCube DOL");
        let data = scratch("core-suite-dolphin-null-gc");
        let paths = RunnerDataPaths::resolve(Some(data.path()), &repo(GC_DOL), &GC, None).unwrap();
        let saves = (system.path(), paths.saves_dir.as_path());
        let mut session = load_dol(core, GC_DOL, saves, &options(&GC, &[NULL]), None);
        assert!(!session.uses_hw_context(), "Null renderer: no GL context");
        let frames = boot(&mut session);
        eprintln!("dolphin null suite: main loop after {frames} frames");
        check_field_counter(&mut session);
        check_pad_words(&mut session);
        check_audio(&mut session);
        check_states_refused(&mut session, &paths);
        check_reset(&mut session);
        assert_eq!(debug_word(&mut session, HID4_AT_BOOT), 0, "GameCube mode");
    }
    {
        eprintln!("dolphin null suite: Wii DOL");
        let saves = scratch("core-suite-dolphin-null-wii");
        let dirs = (system.path(), saves.path());
        let mut session = load_dol(core, WII_DOL, dirs, &options(&WII, &[NULL]), None);
        boot(&mut session);
        check_field_counter(&mut session);
        check_audio(&mut session);
        let hid4 = debug_word(&mut session, HID4_AT_BOOT);
        assert_ne!(hid4, 0, "Wii mode: Dolphin set up Broadway's HID4");
        let (hi, _) = pad_words(&mut session, HeldInput::Joypad(RETRO_A));
        eprintln!("dolphin null suite: Wii HID4 {hid4:08X}, SI IN_HI with A {hi:08X}");
        assert_ne!(
            hi & 0xFFFF_0000,
            0x0180_0000,
            "no GameCube pad on SI in Wii mode"
        );
    }
}

/// Spike values: idle `0x00808080`; A → bit 24; B → bit 25; D-pad up / down / left /
/// right → bits 19 / 18 / 16 / 17; stick right → X byte
/// 0xFF; full right trigger → analog R 0xFF + digital R; right stick up → C-stick Y.
fn check_pad_words(session: &mut CoreSession) {
    let (hi, lo) = pad_words(session, HeldInput::None);
    eprintln!("dolphin null suite: idle IN_HI {hi:08X} IN_LO {lo:08X}");
    assert_eq!(hi, IDLE_HI, "idle IN_HI");
    let (a, _) = pad_words(session, HeldInput::Joypad(RETRO_A));
    assert_eq!(a, IDLE_HI | 1 << 24, "A held");
    let (b, _) = pad_words(session, HeldInput::B);
    assert_eq!(b, IDLE_HI | 1 << 25, "B held");
    // RetroPad Up / Down / Left / Right (ids 4..7) = GC D-pad bits 19 / 18 / 16 / 17.
    for (id, bit, name) in [
        (4, 19, "up"),
        (5, 18, "down"),
        (6, 16, "left"),
        (7, 17, "right"),
    ] {
        let (dpad, _) = pad_words(session, HeldInput::Joypad(id));
        assert_eq!(dpad, IDLE_HI | 1 << bit, "D-pad {name}: {dpad:08X}");
    }
    let (stick, _) = pad_words(session, HeldInput::LeftStickRight);
    assert_eq!(stick & 0xFFFF_00FF, IDLE_HI & 0xFFFF_00FF, "{stick:08X}");
    assert!((stick >> 8) & 0xFF >= 0xF0, "stick X right: {stick:08X}");
    let (hi, lo) = pad_words(session, HeldInput::RightTrigger);
    eprintln!("dolphin null suite: right trigger IN_HI {hi:08X} IN_LO {lo:08X}");
    assert!(lo & 0xFF >= 0xF0, "analog R pulled: {lo:08X}");
    assert_ne!(
        hi & 1 << 21,
        0,
        "full analog R also presses digital R: {hi:08X}"
    );
    let (_, c_up) = pad_words(session, HeldInput::RightStick(0, -0x7FFF));
    assert!((c_up >> 16) & 0xFF >= 0xF0, "C-stick up: {c_up:08X}");
    assert_eq!(pad_words(session, HeldInput::None).0, IDLE_HI, "released");
    assert_eq!(debug_word(session, SI_IN_HI), IDLE_HI);
}

/// Null session: startup resume skipped, slot save / load refused, exit auto-save
/// skipped, existing state files untouched; no state API call (it would crash).
fn check_states_refused(session: &mut CoreSession, paths: &RunnerDataPaths) {
    assert!(
        !session.states_available(),
        "Null renderer: states unavailable"
    );
    assert_eq!(
        session.current_state_size(),
        0,
        "no size query reaches the core"
    );
    assert!(matches!(
        session.serialize(),
        Err(HostError::StatesNeedHwContext)
    ));
    let sidecar = StateSidecar {
        core_name: session.core_info().name.clone(),
        core_version: session.core_info().version.clone(),
        ..StateSidecar::default()
    };
    let json = serde_json::to_vec(&sidecar).unwrap();
    for stem in ["auto", "slot0"] {
        std::fs::write(paths.states_dir.join(format!("{stem}.state")), [7u8; 64]).unwrap();
        std::fs::write(paths.states_dir.join(format!("{stem}.json")), &json).unwrap();
    }
    let saves = GameSaveData::load(paths, session, 0);
    let mut resume = StartupStateLoad::new(
        StateSlotId::Auto,
        system_catalog::SaveStateQuality::Serialized,
    );
    let outcome = resume.attempt(&saves, session).expect("answered at once");
    assert!(!outcome.loaded);
    assert_eq!(outcome.status, COULD_NOT_RESUME);
    assert_eq!(saves.load_state(session).0, STATES_NEED_HW_CONTEXT, "F4");
    let frame = RgbaFrameBuffer::default();
    assert_eq!(
        saves.save_state(session, &frame, None),
        STATES_NEED_HW_CONTEXT,
        "F1"
    );
    let facts = ExitFacts {
        enabled: true,
        states_available: session.states_available(),
        started: true,
        loop_failed: false,
        real_frames: 10_000,
    };
    assert_eq!(
        skip_reason(facts),
        Some("save states need hardware rendering")
    );
    for stem in ["auto", "slot0"] {
        let state = std::fs::read(paths.states_dir.join(format!("{stem}.state"))).unwrap();
        assert_eq!(state, [7u8; 64], "{stem}.state untouched");
    }
    assert!(
        !paths.states_dir.join("slot0.png").exists(),
        "nothing written"
    );
    run(session, HeldInput::None, 10); // the session goes on
}
