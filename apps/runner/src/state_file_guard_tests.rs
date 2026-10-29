//! State guard rules: core name, lenient vs strict version, content tag, length and
//! CRC (incl. the crash window between the sidecar and the state write).

use super::*;

fn core(name: &str, version: &str) -> CoreInfo {
    CoreInfo {
        name: name.into(),
        version: version.into(),
    }
}

fn sidecar(name: &str, version: &str) -> StateSidecar {
    StateSidecar {
        core_name: name.into(),
        core_version: version.into(),
        created_at: 1,
        ..StateSidecar::default()
    }
}

fn error(result: anyhow::Result<()>) -> String {
    format!("{:#}", result.expect_err("refused"))
}

#[test]
fn same_core_any_version_is_accepted_by_lenient_cores() {
    let guard = StateFileGuard::new(&system_catalog::SNES, None);
    let check = |s: &StateSidecar, c: &CoreInfo| guard.check_sidecar(s, c);
    assert!(check(&sidecar("Snes9x", "1.62"), &core("Snes9x", "1.62")).is_ok());
    assert!(check(&sidecar("Snes9x", "1.61"), &core("Snes9x", "1.63")).is_ok());
    let other = error(check(&sidecar("bsnes", "115"), &core("Snes9x", "1.62")));
    assert!(other.contains("bsnes"), "{other}");
}

#[test]
fn strict_core_refuses_another_version_before_the_core() {
    let guard = StateFileGuard::new(&system_catalog::GC, None);
    let dolphin = core("dolphin-emu", "2609-158");
    assert!(guard
        .check_sidecar(&sidecar("dolphin-emu", "2609-158"), &dolphin)
        .is_ok());
    let text = error(guard.check_sidecar(&sidecar("dolphin-emu", "2512-1"), &dolphin));
    assert_eq!(text, "state was made by another Dolphin version");
    // A sidecar without a version is not refused (unknown).
    assert!(guard
        .check_sidecar(&sidecar("dolphin-emu", ""), &dolphin)
        .is_ok());
}

#[test]
fn other_disc_state_is_refused_untagged_states_load() {
    let disc1 = StateFileGuard::new(&system_catalog::GC, Some("GALE01:0:0".into()));
    let dolphin = core("dolphin-emu", "1");
    let mut saved = sidecar("dolphin-emu", "1");
    StateFileGuard::new(&system_catalog::GC, Some("GALE01:1:0".into())).stamp(&mut saved, b"s");
    let text = error(disc1.check_sidecar(&saved, &dolphin));
    assert_eq!(text, "state was made from another disc or revision");
    saved.content_tag = None; // written before M14
    assert!(disc1.check_sidecar(&saved, &dolphin).is_ok());
    // Same ID, disc and revision (ISO vs RVZ copy): accepted.
    disc1.stamp(&mut saved, b"s");
    assert!(disc1.check_sidecar(&saved, &dolphin).is_ok());
}

#[test]
fn stamped_length_and_crc_catch_truncated_and_flipped_states() {
    let guard = StateFileGuard::default();
    let state = b"state bytes".to_vec();
    let mut saved = sidecar("Snes9x", "1");
    guard.stamp(&mut saved, &state);
    assert_eq!(saved.state_len, Some(state.len() as u64));
    assert_eq!(saved.content_tag, None, "untagged system");
    assert!(StateFileGuard::check_len(&saved, state.len() as u64).is_ok());
    assert!(StateFileGuard::check_crc(&saved, &state).is_ok());
    let truncated = error(StateFileGuard::check_len(&saved, 4));
    assert!(truncated.starts_with(STATE_DAMAGED), "{truncated}");
    let mut flipped = state.clone();
    flipped[0] ^= 1;
    let text = error(StateFileGuard::check_crc(&saved, &flipped));
    assert!(text.starts_with(STATE_DAMAGED), "{text}");
}

#[test]
fn crash_between_sidecar_and_state_write_reads_as_damaged() {
    // New sidecar on disk, old state still there (the write order is sidecar first).
    let mut new_sidecar = sidecar("Snes9x", "1");
    StateFileGuard::default().stamp(&mut new_sidecar, b"new state");
    let old_state = b"old state!";
    let text = error(
        StateFileGuard::check_len(&new_sidecar, old_state.len() as u64)
            .and_then(|()| StateFileGuard::check_crc(&new_sidecar, old_state)),
    );
    assert!(text.starts_with(STATE_DAMAGED), "{text}");
}

#[test]
fn old_sidecar_without_length_or_crc_is_not_checked() {
    let old = sidecar("Snes9x", "1");
    assert!(StateFileGuard::check_len(&old, 123).is_ok());
    assert!(StateFileGuard::check_crc(&old, b"anything").is_ok());
}
