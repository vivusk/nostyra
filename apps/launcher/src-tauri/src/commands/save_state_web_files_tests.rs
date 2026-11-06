//! A save folder shared with the web edition: its `web-slotN.*` / `web-auto.*`
//! states (other core builds), `save.web.srm` and Chrome `*.crswap` swap files are
//! never listed, loaded or deleted by the desktop.

use super::*;

const WEB_FILES: [&str; 10] = [
    "web-slot0.state",
    "web-slot0.json",
    "web-slot0.png",
    "web-slot3.state",
    "web-auto.state",
    "web-auto.json",
    "web-auto.png",
    "save.web.srm",
    "slot3.state.crswap",
    "auto.state.crswap",
];

fn folder_with_web_files() -> tempfile::TempDir {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-slots-web-")
        .tempdir()
        .unwrap();
    for name in WEB_FILES {
        std::fs::write(temp.path().join(name), b"web").unwrap();
    }
    temp
}

#[test]
fn web_states_and_swap_files_are_not_listed() {
    let temp = folder_with_web_files();
    let dir = temp.path();
    assert!(list_slots(dir, &StateLoadRule::default()).is_empty());
    std::fs::write(dir.join("slot3.state"), b"desktop").unwrap();
    let slots: Vec<StateSlotId> = list_slots(dir, &StateLoadRule::default())
        .into_iter()
        .map(|slot| slot.slot)
        .collect();
    assert_eq!(slots, [StateSlotId::Numbered(3)]);
}

#[test]
fn deleting_a_desktop_slot_keeps_the_web_files() {
    let temp = folder_with_web_files();
    let dir = temp.path();
    for name in ["slot0.state", "slot0.json", "auto.state", "auto.json"] {
        std::fs::write(dir.join(name), b"desktop").unwrap();
    }
    delete_slot_files(dir, StateSlotId::Numbered(0)).unwrap();
    delete_slot_files(dir, StateSlotId::Auto).unwrap();
    assert!(!dir.join("slot0.state").exists() && !dir.join("auto.state").exists());
    for name in WEB_FILES {
        assert!(dir.join(name).exists(), "{name}");
    }
}
