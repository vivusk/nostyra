//! `SystemSpec::ra_console_id`: rcheevos console ids (M15 plan table).

use std::collections::HashSet;

use crate::{ALL_SYSTEMS, GC, WII};

#[test]
fn ra_console_ids_match_rcheevos() {
    let expected: [(&str, Option<u8>); 16] = [
        ("nes", Some(7)),
        ("snes", Some(3)),
        ("gb", Some(4)),
        ("gbc", Some(6)),
        ("gba", Some(5)),
        ("genesis", Some(1)),
        ("sms", Some(11)),
        ("gg", Some(15)),
        ("sg1000", Some(33)),
        ("psx", Some(12)),
        ("n64", Some(2)),
        ("dreamcast", Some(40)),
        ("psp", Some(41)),
        ("nds", Some(18)),
        ("gc", None),
        ("wii", None),
    ];
    for (id, console) in expected {
        let spec = ALL_SYSTEMS.iter().find(|s| s.id == id).unwrap();
        assert_eq!(spec.ra_console_id, console, "{id}");
    }
    assert_eq!(ALL_SYSTEMS.len(), expected.len());
}

#[test]
fn fourteen_systems_have_unique_ids_and_gamecube_wii_have_none() {
    let ids: Vec<u8> = ALL_SYSTEMS.iter().filter_map(|s| s.ra_console_id).collect();
    assert_eq!(ids.len(), 14);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 14, "unique");
    assert_eq!((GC.ra_console_id, WII.ra_console_id), (None, None));
}
