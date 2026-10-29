//! `StartupStateLoad::attempt_on` driven by a fake game.

use super::*;

const PRE_BOOT: usize = 128 << 20;

struct Fake {
    requery: bool,
    must_init: bool,
    size: usize,
    rejects: bool,
    loads: u32,
    available: bool,
    /// State-size queries (a core call, segfault for Dolphin without a context).
    size_queries: u32,
    hardcore: bool,
}

impl Fake {
    fn new(requery: bool, rejects: bool) -> Self {
        Self {
            requery,
            must_init: !requery,
            size: PRE_BOOT,
            rejects,
            loads: 0,
            available: true,
            size_queries: 0,
            hardcore: false,
        }
    }
}

impl StartupTarget for Fake {
    fn hardcore_active(&self) -> bool {
        self.hardcore
    }
    fn has_state(&self, _: StateSlotId) -> bool {
        true
    }
    fn states_available(&self) -> bool {
        self.available
    }
    fn state_size_requery(&self) -> bool {
        self.requery
    }
    fn current_state_size(&mut self) -> usize {
        self.size_queries += 1;
        self.size
    }
    fn must_initialize(&self) -> bool {
        self.must_init
    }
    fn load_slot(&mut self, _: StateSlotId) -> anyhow::Result<Option<String>> {
        self.loads += 1;
        if self.rejects {
            Err(anyhow::Error::new(HostError::UnserializeFailed))
        } else {
            Ok(Some("00ff".into()))
        }
    }
}

fn auto() -> StartupStateLoad {
    StartupStateLoad::new(StateSlotId::Auto, SaveStateQuality::Serialized)
}

#[test]
fn requery_core_waits_for_boot_without_loading() {
    let (mut load, mut game) = (auto(), Fake::new(true, false));
    assert!(load.attempt_on(&mut game).is_none());
    assert!(load.attempt_on(&mut game).is_none());
    assert_eq!(game.loads, 0, "a pre-boot load would be accepted blindly");
    game.size = 40 << 20;
    let out = load.attempt_on(&mut game).unwrap();
    assert!(out.loaded && out.status == RESUMED);
    assert_eq!(
        out.progress.as_deref(),
        Some("00ff"),
        "sidecar progress passed on"
    );
    assert_eq!(game.loads, 1);
}

#[test]
fn requery_core_rejection_after_boot_is_one_attempt_and_not_resumed() {
    let (mut load, mut game) = (auto(), Fake::new(true, true));
    assert!(load.attempt_on(&mut game).is_none());
    game.size = 40 << 20;
    let out = load.attempt_on(&mut game).unwrap();
    assert!(!out.loaded && out.progress.is_none());
    assert_eq!(out.status, COULD_NOT_RESUME);
    assert_eq!(game.loads, 1, "no retry");
    assert_eq!(load.retries_left, MAX_RETRY_FRAMES);
}

#[test]
fn other_cores_retry_rejections_up_to_the_cap() {
    let (mut load, mut game) = (auto(), Fake::new(false, true));
    for _ in 0..MAX_RETRY_FRAMES {
        assert!(load.attempt_on(&mut game).is_none());
    }
    let out = load.attempt_on(&mut game).unwrap();
    assert_eq!(out.status, COULD_NOT_RESUME);
    assert_eq!(game.loads, MAX_RETRY_FRAMES + 1);
}

#[test]
fn unavailable_states_fail_at_once_without_a_core_call() {
    for slot in [StateSlotId::Auto, StateSlotId::Numbered(2)] {
        let mut load = StartupStateLoad::new(slot, SaveStateQuality::Serialized);
        let mut game = Fake::new(true, false);
        game.available = false;
        let out = load.attempt_on(&mut game).expect("final at once");
        assert!(!out.loaded);
        let expected = match slot {
            StateSlotId::Auto => COULD_NOT_RESUME,
            StateSlotId::Numbered(_) => STATES_NEED_HW_CONTEXT,
        };
        assert_eq!(out.status, expected);
        assert_eq!((game.loads, game.size_queries), (0, 0), "no state API call");
    }
}

#[test]
fn hardcore_skips_every_startup_load_without_a_core_call() {
    for slot in [StateSlotId::Auto, StateSlotId::Numbered(2)] {
        for requery in [false, true] {
            let mut load = StartupStateLoad::new(slot, SaveStateQuality::Deterministic);
            let mut game = Fake::new(requery, false);
            game.hardcore = true;
            let out = load.attempt_on(&mut game).expect("final at once");
            assert!(!out.loaded && out.progress.is_none());
            assert_eq!(
                out.status, STATE_LOAD_REFUSED,
                "same text for the resume point"
            );
            assert_eq!((game.loads, game.size_queries), (0, 0), "no state API call");
        }
    }
}

#[test]
fn requery_core_gets_one_attempt_others_retry() {
    assert!(
        !retries_rejection(true, true, true),
        "re-query core: final answer"
    );
    assert!(retries_rejection(false, true, false), "Serialized (Mupen)");
    assert!(retries_rejection(false, false, true), "MUST_INITIALIZE");
    assert!(!retries_rejection(false, false, false));
}
