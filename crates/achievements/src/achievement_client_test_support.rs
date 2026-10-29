//! Shared fixtures of the `AchievementClient` tests: a client over a fake server with
//! 4 KiB of test RAM, and helpers that pump the worker until an op completes.

#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::fake_game_set_json::fake_game_set_json;
use crate::fake_server_transport::FakeTransport;
use crate::{AchievementClient, ClientCompletion, ClientOp, MemoryReader, RequestDispatch};

pub(crate) const HASH: &str = "0123456789abcdef0123456789abcdef";
pub(crate) const ACH_BYTE: u32 = 101; // 0xH0010=5
pub(crate) const ACH_HITS: u32 = 102; // 0xH0030=1.10.
pub(crate) const LBOARD: u32 = 201;

/// Test session: client + its fake server + 4 KiB of RAM the memory reader serves.
pub(crate) struct Session {
    pub client: AchievementClient,
    pub fake: Arc<FakeTransport>,
    pub ram: Rc<RefCell<Vec<u8>>>,
}

pub(crate) fn session(dispatch: RequestDispatch, with_set: bool) -> Session {
    let fake = Arc::new(FakeTransport::new());
    if with_set {
        let set = fake_game_set_json(
            1234,
            "Test Game",
            &[(ACH_BYTE, "0xH0010=5"), (ACH_HITS, "0xH0030=1.10.")],
            &[(
                LBOARD,
                "STA:0xH0020=1::CAN:0xH0021=1::SUB:0xH0020=2::VAL:0xH0022",
            )],
        );
        fake.respond("achievementsets", 200, &set);
    }
    let ram = Rc::new(RefCell::new(vec![0u8; 0x1000]));
    let reader_ram = ram.clone();
    let memory: MemoryReader = Box::new(move |address, buffer| {
        let ram = reader_ram.borrow();
        let start = address as usize;
        match ram.get(start..start + buffer.len()) {
            Some(bytes) => {
                buffer.copy_from_slice(bytes);
                buffer.len() as u32
            }
            None => 0,
        }
    });
    let client = AchievementClient::new(fake.clone(), dispatch, memory).unwrap();
    Session { client, fake, ram }
}

/// Drains worker results until `op` completes (5 s cap); returns its completion.
pub(crate) fn wait_for(client: &AchievementClient, op: ClientOp) -> ClientCompletion {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        client.complete_ready_requests();
        seen.extend(client.drain_completions());
        if let Some(done) = seen.iter().find(|c| c.op == op) {
            return done.clone();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("{op:?} never completed; saw {seen:?}");
}

pub(crate) fn login_and_load(s: &Session) {
    s.client.begin_login_with_token("Tester", "tok123");
    assert!(wait_for(&s.client, ClientOp::LoginWithToken).is_ok());
    s.client.begin_load_game(HASH);
    assert!(wait_for(&s.client, ClientOp::LoadGame).is_ok());
}
