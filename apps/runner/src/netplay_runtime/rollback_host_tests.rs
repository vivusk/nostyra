//! `CoreRollbackHost` on the fake core: call order, AV table, forward-path rule (F20).

use super::*;
use crate::netplay_runtime::fake_netplay_core::FakeNetplayCore;
use crate::netplay_runtime::netplay_core::run_warmup;

fn inputs(status: InputStatus) -> Vec<(NetplayInput, InputStatus)> {
    vec![
        (NetplayInput::default(), InputStatus::Confirmed),
        (NetplayInput::default(), status),
    ]
}

#[test]
fn av_table_matches_the_plan() {
    assert_eq!(av_for(false, false), AvEnable::NONE);
    assert_eq!(av_for(true, false), AUDIO_ONLY);
    assert_eq!(av_for(true, true), AvEnable::ALL);
    assert_eq!(
        av_for(false, true),
        AvEnable {
            video: true,
            audio: false
        }
    );
}

#[test]
fn save_load_advance_and_forward_path_hooks() {
    let mut core = FakeNetplayCore::new(0);
    let mut history = FrameCrcs::default();
    let mut host = CoreRollbackHost {
        core: &mut core,
        ram_crc: &mut history,
        spectator: false,
    };
    let mut buf = Vec::new();
    let checksum = host.save(0, &mut buf).unwrap();
    host.advance(0, &inputs(InputStatus::Predicted), true, true);
    assert_ne!(host.core.ram_checksum(), checksum, "RAM moved");
    // Re-simulation of frame 0 after a load: no hook.
    host.load(0, &buf).unwrap();
    host.advance(0, &inputs(InputStatus::Confirmed), false, false);
    host.advance(1, &inputs(InputStatus::Predicted), true, true);
    assert_eq!(history.ram.get(0), Some(checksum as u32));
    assert_eq!(core.log, ["save", "run", "load", "run", "run"]);
    assert_eq!(core.new_frame_hooks, 2, "frame 0 once, frame 1 once");
    assert_eq!(core.presented_hooks, 2);
    assert_eq!(core.avs, [AvEnable::ALL, AvEnable::NONE, AvEnable::ALL]);
}

#[test]
fn spectator_records_after_advance() {
    let mut core = FakeNetplayCore::new(0);
    let mut history = FrameCrcs::default();
    let mut host = CoreRollbackHost {
        core: &mut core,
        ram_crc: &mut history,
        spectator: true,
    };
    host.advance(0, &inputs(InputStatus::Confirmed), true, true);
    let after = host.core.ram_checksum() as u32;
    assert_eq!(history.ram.get(1), Some(after), "start of frame 1");
}

#[test]
fn warmup_counts_frames_until_the_core_serializes() {
    assert_eq!(run_warmup(&mut FakeNetplayCore::new(0)), Some(0));
    let mut core = FakeNetplayCore::new(3);
    assert_eq!(run_warmup(&mut core), Some(3));
    assert!(core.avs.iter().all(|av| *av == AvEnable::NONE));
    assert_eq!(
        run_warmup(&mut FakeNetplayCore::new(1_000)),
        None,
        "cap 300"
    );
}
