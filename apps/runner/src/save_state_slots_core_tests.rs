//! `SaveStateSlots` against a real core + test ROM. Called from
//! `runner_real_core_suite` (one core session per process).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use libretro_host::{CoreSession, NullAudioSink, NullInputSource, NullVideoSink};

use crate::hardcore_gate::{HardcoreGate, STATE_LOAD_REFUSED};
use crate::save_state_slots::{failure_status, SaveStateSlots, StateSidecar, StateSlotId};
use crate::sram_persistence::sram_crc32;

const SLOT0: StateSlotId = StateSlotId::Numbered(0);

fn run(session: &mut CoreSession, video: &mut NullVideoSink, frames: usize) -> Vec<u64> {
    let (mut audio, mut input) = (NullAudioSink::default(), NullInputSource);
    (0..frames)
        .map(|_| {
            session.run_frame(video, &mut audio, &mut input).unwrap();
            video.last_checksum
        })
        .collect()
}

fn error_of(slots: &SaveStateSlots, session: &mut CoreSession) -> String {
    format!(
        "{:#}",
        slots.load(session, SLOT0).expect_err("load must fail")
    )
}

fn write_sidecar(dir: &Path, core_name: &str, core_version: &str) {
    let sidecar = StateSidecar {
        core_name: core_name.into(),
        core_version: core_version.into(),
        created_at: 1,
        ..StateSidecar::default()
    };
    std::fs::write(
        dir.join("slot0.json"),
        serde_json::to_vec(&sidecar).unwrap(),
    )
    .unwrap();
}

/// Frames after a load against the frames the first run showed after the save.
/// `exact_replay`: the same frames. Otherwise (Mupen64Plus-Next: serialize runs the
/// core to a safe point, so replay is not checksum-exact) the replay must show the
/// first run's frames shifted by at most one frame.
fn assert_replay(first: &[u64], replay: &[u64], exact_replay: bool) {
    if exact_replay {
        assert_eq!(first, replay, "frames after the load match the first run");
        return;
    }
    let shifted = |a: &[u64], b: &[u64]| a[1..] == b[..b.len() - 1];
    assert!(
        first == replay || shifted(first, replay) || shifted(replay, first),
        "frames after the load match the first run +-1 frame: {first:x?} vs {replay:x?}"
    );
}

/// The registry's largest state cap: these checks run on every core's states.
fn largest_cap() -> u64 {
    let caps = system_catalog::CORES
        .iter()
        .map(|core| core.max_state_bytes);
    caps.max()
        .unwrap_or(system_catalog::DEFAULT_MAX_STATE_BYTES)
}

/// Save/load round trip and every rejection path; `dir` is an empty scratch folder.
/// `exact_replay`: see [`assert_replay`].
pub(crate) fn check_state_slots(session: &mut CoreSession, dir: &Path, exact_replay: bool) {
    let dir = dir.to_path_buf();
    let slots = SaveStateSlots::new(dir.clone(), largest_cap());
    let mut video = NullVideoSink::default();

    assert!(error_of(&slots, session).contains("empty"));

    // Save, advance, load, replay: frames after the load match the original run.
    run(session, &mut video, 30);
    slots.save(session, SLOT0, None).unwrap();
    let sidecar: StateSidecar =
        serde_json::from_slice(&std::fs::read(dir.join("slot0.json")).unwrap()).unwrap();
    assert_eq!(sidecar.core_name, session.core_info().name);
    assert_eq!(sidecar.core_version, session.core_info().version);
    assert!(dir.join("slot0.state").metadata().unwrap().len() > 0);
    let first = run(session, &mut video, 30);
    assert_eq!(
        slots.load(session, SLOT0).unwrap(),
        None,
        "saved without progress"
    );
    assert_replay(&first, &run(session, &mut video, 30), exact_replay);

    // Flipped byte / truncated file: refused by the sidecar length + CRC, no core call.
    let good = std::fs::read(dir.join("slot0.state")).unwrap();
    let mut flipped = good.clone();
    flipped[good.len() / 2] ^= 0xFF;
    for bad in [&flipped[..], &good[..good.len() - 1]] {
        std::fs::write(dir.join("slot0.state"), bad).unwrap();
        assert!(error_of(&slots, session).contains("damaged"));
    }
    std::fs::write(dir.join("slot0.state"), &good).unwrap();

    // Different core version: warn but load. Different core name: rejected.
    write_sidecar(&dir, &session.core_info().name, "0.0-other");
    slots.load(session, SLOT0).unwrap();
    write_sidecar(&dir, "bsnes", "1");
    assert!(error_of(&slots, session).contains("bsnes"));

    // Corrupt / missing sidecar fail before the core is touched.
    std::fs::write(dir.join("slot0.json"), b"{ not json").unwrap();
    assert!(error_of(&slots, session).contains("corrupt state metadata"));
    std::fs::remove_file(dir.join("slot0.json")).unwrap();
    assert!(error_of(&slots, session).contains("missing state metadata"));

    // Valid sidecar but garbage state: the core rejects it (for Mupen64Plus-Next the
    // host does, by size: its `retro_unserialize` hangs on a bad state).
    write_sidecar(&dir, &session.core_info().name, "x");
    std::fs::write(dir.join("slot0.state"), [1u8, 2, 3]).unwrap();
    assert!(error_of(&slots, session).contains("core rejected state"));

    // Oversized state file is refused without reading it.
    std::fs::File::create(dir.join("slot0.state"))
        .unwrap()
        .set_len(largest_cap() + 1)
        .unwrap();
    assert!(error_of(&slots, session).contains("limit"));

    check_auto_slot(session, &dir, exact_replay);
}

/// The `auto` resume point: own files, SRAM / patch CRCs in the sidecar, restore.
fn check_auto_slot(session: &mut CoreSession, dir: &Path, exact_replay: bool) {
    let mut slots = SaveStateSlots::new(dir.to_path_buf(), largest_cap());
    slots.set_rom_patch_crc32(Some(0xDEAD_BEEF));
    let mut video = NullVideoSink::default();
    run(session, &mut video, 10);
    slots
        .save(session, StateSlotId::Auto, Some("00ab".into()))
        .unwrap();
    let sidecar: StateSidecar =
        serde_json::from_slice(&std::fs::read(dir.join("auto.json")).unwrap()).unwrap();
    assert!(dir.join("auto.state").metadata().unwrap().len() > 0);
    assert_eq!(sidecar.sram_crc32, sram_crc32(session));
    let has_sram = session.save_ram_mut().is_some(); // not on the SG-1000 test cart
    assert_eq!(sidecar.sram_crc32.is_some(), has_sram, "SRAM CRC recorded");
    assert_eq!(sidecar.rom_patch_crc32, Some(0xDEAD_BEEF));
    assert_eq!(sidecar.achievements_progress.as_deref(), Some("00ab"));
    let first = run(session, &mut video, 30);
    let progress = slots.load(session, StateSlotId::Auto).unwrap();
    assert_eq!(
        progress.as_deref(),
        Some("00ab"),
        "progress returned to the caller"
    );
    assert_replay(&first, &run(session, &mut video, 30), exact_replay);

    // Hardcore: saving still works, every load is refused before the slot's files
    // (an empty slot answers the refusal, not "empty") and before the core.
    slots.set_hardcore_gate(HardcoreGate::new(true));
    slots.save(session, StateSlotId::Auto, None).unwrap();
    for id in [StateSlotId::Auto, StateSlotId::Numbered(9)] {
        let err = slots.load(session, id).expect_err("refused in hardcore");
        assert_eq!(failure_status(&err, "Load failed"), STATE_LOAD_REFUSED);
    }
}
