//! `set_controller_port_devices` against the fake core: one call per listed port, in
//! order, after load; an empty list never reaches the core.

use crate::core_session_hw_render::fake_core_tests::start;
use crate::ffi_types::{RETRO_DEVICE_JOYPAD, RETRO_DEVICE_NONE};
use crate::hw_render_test_fakes::{log_take, serial, FakeProvider};

fn port_calls() -> Vec<String> {
    log_take()
        .into_iter()
        .filter(|e| e.starts_with("port "))
        .collect()
}

#[test]
fn every_listed_port_is_set_in_order() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    log_take();
    session.set_controller_port_devices(&[
        RETRO_DEVICE_JOYPAD,
        RETRO_DEVICE_JOYPAD,
        RETRO_DEVICE_NONE,
        RETRO_DEVICE_NONE,
    ]);
    assert_eq!(
        port_calls(),
        ["port 0=1", "port 1=1", "port 2=0", "port 3=0"]
    );
    drop(session);
}

#[test]
fn empty_list_makes_no_core_call() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    session.set_controller_port_devices(&[]);
    drop(session);
    assert!(port_calls().is_empty(), "no port call for existing cores");
}
