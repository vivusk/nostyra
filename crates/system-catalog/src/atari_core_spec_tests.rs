//! Final Atari core specs from the M17 spike (`atari_core_specs`).

use crate::{core, SaveStateQuality, DEFAULT_MAX_STATE_BYTES};

#[test]
fn atari_cores_follow_the_spike() {
    for id in ["stella", "prosystem", "handy"] {
        let spec = core(id).unwrap();
        assert_eq!(spec.savestates, SaveStateQuality::Deterministic, "{id}");
        assert!(
            spec.port_devices.is_empty() && spec.process_env.is_empty(),
            "{id}"
        );
        assert!(
            !spec.requery_state_size && !spec.keep_library_loaded,
            "{id}"
        );
        assert_eq!(
            spec.max_state_bytes, DEFAULT_MAX_STATE_BYTES,
            "1 MiB Stella fits"
        );
    }
    let handy = core("handy").unwrap();
    assert!(handy.stage_content && handy.strict_state_version && handy.exact_state_size);
    for id in ["stella", "prosystem"] {
        let spec = core(id).unwrap();
        assert!(!spec.stage_content && !spec.strict_state_version, "{id}");
        assert!(
            !spec.exact_state_size,
            "{id}: refuses or sizes states itself"
        );
    }
}
