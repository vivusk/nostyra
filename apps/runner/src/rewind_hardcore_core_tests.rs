//! Rewind under achievement hardcore against a real core (called from
//! `rewind_core_tests::check_rewind`, inside `runner_real_core_suite`).

use config_layers::RewindConfig;
use libretro_host::{CoreSession, NullAudioSink, NullInputSource, NullVideoSink};
use system_catalog::SaveStateQuality::Deterministic;

use crate::emulation_main_loop::rewind_playback::Rewind;
use crate::hardcore_gate::{HardcoreGate, REWIND_REFUSED};

/// Ten real frames with the loop's capture hook.
fn frames(rewind: &mut Rewind, session: &mut CoreSession) {
    let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
    for _ in 0..10 {
        let ran = session.run_frame(&mut video, &mut audio, &mut NullInputSource);
        assert!(ran.is_ok(), "frame: {ran:?}");
        rewind.after_real_frame(session, None, || None);
    }
}

/// Hardcore: no capture, no playback, the menu reason; once the session drops to
/// softcore, captures start again.
pub(crate) fn check_hardcore_rewind(session: &mut CoreSession, config: &RewindConfig) {
    let gate = HardcoreGate::new(true);
    let mut rewind = Rewind::new(config, session, Deterministic);
    rewind.set_hardcore_gate(gate.clone());
    frames(&mut rewind, session);
    assert!(
        rewind.pop_captures(1).is_none(),
        "nothing captured in hardcore"
    );
    assert_eq!(rewind.set_held(true), None, "no playback");
    assert!(!rewind.is_rewinding());
    assert_eq!(rewind.unavailable_reason(), Some(REWIND_REFUSED));
    gate.lift();
    rewind.set_held(false);
    frames(&mut rewind, session);
    assert!(
        rewind.pop_captures(1).is_some(),
        "captures after the drop to softcore"
    );
    assert_eq!(rewind.unavailable_reason(), None);
}
