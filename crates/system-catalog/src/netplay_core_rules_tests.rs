use super::*;
use crate::{
    core, CORES, DEVICE_GPGX_MASTER_TAP, DEVICE_GPGX_TEAM_PLAYER_3B, DEVICE_JOYPAD,
    DEVICE_JOYPAD_MULTITAP,
};

fn spec(id: &str) -> &'static CoreSpec {
    core(id).unwrap_or_else(|| panic!("core {id}"))
}

fn registry_check(id: &str) -> Result<NetplaySupport, NetplayRefusal> {
    let spec = spec(id);
    netplay_eligibility(spec, spec.savestates, None)
}

#[test]
fn every_core_has_the_spike_00_verdict() {
    use NetplayRefusal::{NotSupported, SerializedStates};
    use NetplaySupport::{Experimental, Yes};
    let expected = [
        ("snes9x", Ok(Yes)),
        ("mesen", Ok(Yes)),
        ("genesis_plus_gx", Ok(Yes)),
        ("stella", Ok(Yes)),
        ("mgba", Ok(Yes)),
        ("pcsx-rearmed", Ok(Experimental)),
        ("prosystem", Ok(Experimental)),
        ("handy", Err(NotSupported)),
        ("melonds_ds", Err(NotSupported)),
        ("mupen64plus-next", Err(NotSupported)),
        ("flycast", Err(NotSupported)),
        ("dolphin-emu", Err(NotSupported)),
        ("ppsspp", Err(NotSupported)),
        ("pcee2", Err(NotSupported)),
    ];
    assert_eq!(expected.len(), CORES.len(), "every core listed");
    for (id, verdict) in expected {
        assert_eq!(registry_check(id), verdict, "{id}");
    }
    // A GL core flagged by mistake would still be refused by its state quality.
    let flagged = CoreSpec {
        netplay: Yes,
        ..*spec("mupen64plus-next")
    };
    let refusal = netplay_eligibility(&flagged, flagged.savestates, None);
    assert_eq!(refusal, Err(SerializedStates));
}

#[test]
fn runner_check_refuses_incomplete_states_reported_after_load() {
    let snes = spec("snes9x");
    let incomplete = Some(NetplayQuirks { incomplete: true });
    let refusal = netplay_eligibility(snes, snes.savestates, incomplete);
    assert_eq!(refusal, Err(NetplayRefusal::IncompleteStates));
    let fine = Some(NetplayQuirks::default());
    let support = netplay_eligibility(snes, snes.savestates, fine);
    assert_eq!(support, Ok(NetplaySupport::Yes));
}

#[test]
fn room_size_per_system() {
    let gpgx = spec("genesis_plus_gx");
    let rooms = [
        (spec("snes9x"), "snes", 4),
        (spec("mesen"), "nes", 4),
        (gpgx, "genesis", 4),
        (gpgx, "sms", 4),
        (gpgx, "gg", 1),
        (gpgx, "sg1000", DEFAULT_NETPLAY_PLAYERS),
        (spec("pcsx-rearmed"), "psx", 4),
        (spec("stella"), "atari2600", 2),
        (spec("prosystem"), "atari7800", 2),
        (spec("mgba"), "gba", 1),
        (spec("mgba"), "gb", 1),
    ];
    for (core, system, players) in rooms {
        assert_eq!(netplay_max_players(core, system), players, "{system}");
    }
}

#[test]
fn two_player_rooms_keep_the_default_setup() {
    for core in CORES {
        for system in core.systems {
            for players in [1, 2] {
                let devices = port_devices_for(core, system, players);
                assert_eq!(devices, core.port_devices, "{system} {players}");
                assert!(multitap_options_for(core, system, players).is_empty());
            }
        }
    }
}

#[test]
fn four_player_rooms_get_the_multitap_devices_and_options() {
    let gpgx = spec("genesis_plus_gx");
    let multitap = [
        (
            spec("snes9x"),
            "snes",
            &[DEVICE_JOYPAD, DEVICE_JOYPAD_MULTITAP][..],
        ),
        (
            spec("mesen"),
            "nes",
            &[DEVICE_JOYPAD, DEVICE_JOYPAD, 257, 257][..],
        ),
        (
            gpgx,
            "genesis",
            &[DEVICE_GPGX_TEAM_PLAYER_3B, DEVICE_JOYPAD][..],
        ),
        (gpgx, "sms", &[DEVICE_GPGX_MASTER_TAP, DEVICE_JOYPAD][..]),
        (gpgx, "gg", &[][..]),
        (spec("stella"), "atari2600", &[][..]),
    ];
    for (core, system, devices) in multitap {
        assert_eq!(port_devices_for(core, system, 4), devices, "{system}");
        assert!(multitap_options_for(core, system, 4).is_empty(), "{system}");
    }
    assert_eq!(DEVICE_GPGX_TEAM_PLAYER_3B, 1537);
    assert_eq!(DEVICE_GPGX_MASTER_TAP, 2049);
    let pcsx = spec("pcsx-rearmed");
    assert!(port_devices_for(pcsx, "psx", 4).is_empty(), "option only");
    let options = multitap_options_for(pcsx, "psx", 3);
    assert_eq!(options, [("pcsx_rearmed_multitap", "port 1")]);
}

#[test]
fn multitap_option_keys_are_not_already_fixed_options() {
    for core in CORES {
        for entry in core.multitap {
            for (key, _) in entry.options {
                let fixed = core.fixed_options.iter().any(|(k, _)| k == key);
                assert!(!fixed, "{} {key}", core.id);
            }
            for system in entry.systems {
                assert!(core.systems.contains(system), "{} {system}", core.id);
            }
        }
    }
}
