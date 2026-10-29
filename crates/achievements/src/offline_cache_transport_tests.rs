#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::*;
use crate::fake_server_transport::{FakeTransport, FAKE_LOGIN_JSON, FAKE_UNKNOWN_GAME_JSON};
use crate::pending_unlock_journal::PendingEntry;

const HASH: &str = "0123456789abcdef0123456789abcdef";
const SET: &str = r#"{"Success":true,"GameId":1234,"Title":"T","Sets":[]}"#;
const SESSION: &str = r#"{"Success":true,"Unlocks":[{"ID":1,"When":5}],"HardcoreUnlocks":[]}"#;
const LOGIN: &str = "r=login2&u=Tester&t=tok123";
const SETS: &str = "r=achievementsets&u=Tester&t=tok123&m=0123456789abcdef0123456789abcdef";
const START: &str =
    "r=startsession&u=Tester&t=tok123&g=1234&h=0&m=0123456789abcdef0123456789abcdef";

struct Setup {
    data: tempfile::TempDir,
    fake: Arc<FakeTransport>,
    transport: OfflineCacheTransport,
}

fn setup() -> Setup {
    let data = tempfile::tempdir().unwrap();
    let fake = Arc::new(FakeTransport::new());
    fake.respond("achievementsets", 200, SET);
    fake.respond("startsession", 200, SESSION);
    let transport = OfflineCacheTransport::new(fake.clone(), data.path());
    Setup {
        data,
        fake,
        transport,
    }
}

fn call(transport: &dyn ServerTransport, post: &str) -> ServerResponse {
    transport.call(&ServerRequest {
        url: "https://retroachievements.org/dorequest.php".into(),
        post_data: post.into(),
        content_type: String::new(),
    })
}

/// A second transport over the same data dir; its server is offline.
fn offline(setup: &Setup) -> (Arc<FakeTransport>, OfflineCacheTransport) {
    let fake = Arc::new(FakeTransport::new());
    for kind in "login2 achievementsets startsession awardachievement".split(' ') {
        fake.respond(kind, -2, "offline");
    }
    let transport = OfflineCacheTransport::new(fake.clone(), setup.data.path());
    (fake, transport)
}

fn online_run(setup: &Setup) {
    for post in [LOGIN, SETS, START] {
        assert_eq!(call(&setup.transport, post).status, 200);
    }
}

fn session_ids(body: &str) -> Vec<u64> {
    let json: Value = serde_json::from_str(body).unwrap();
    let ids = json["Unlocks"].as_array().unwrap();
    ids.iter().map(|e| e["ID"].as_u64().unwrap()).collect()
}

#[test]
fn online_answers_are_stored_and_served_offline_with_sticky_short_circuit() {
    let setup = setup();
    online_run(&setup);
    assert!(!setup.transport.state().any_served());
    let (fake, transport) = offline(&setup);
    let login = call(&transport, LOGIN);
    assert_eq!((login.status, login.body.as_str()), (200, FAKE_LOGIN_JSON));
    let state = transport.state();
    assert!(
        state.any_served() && !state.set_served(),
        "login alone: no sync claim"
    );
    assert_eq!(call(&transport, SETS).body, SET);
    assert!(state.set_served());
    assert_eq!(session_ids(&call(&transport, START).body), [1]);
    assert_eq!(
        fake.kinds(),
        ["login2"],
        "sticky: later cacheable kinds skip the server"
    );
    call(&transport, "r=awardachievement&u=Tester&a=1&h=0&m=00");
    call(&transport, "r=ping&u=Tester&g=1234");
    assert_eq!(fake.kinds(), ["login2", "awardachievement", "ping"]);
}

#[test]
fn gateway_status_counts_as_offline() {
    let setup = setup();
    online_run(&setup);
    let (fake, transport) = offline(&setup);
    fake.respond("login2", 503, "<html>down</html>");
    assert_eq!(call(&transport, LOGIN).body, FAKE_LOGIN_JSON);
}

#[test]
fn error_answers_are_neither_stored_nor_served_over() {
    let setup = setup();
    let refused = r#"{"Success":false,"Error":"Invalid token"}"#;
    setup.fake.respond("login2", 401, refused);
    setup.fake.respond("achievementsets", 200, refused);
    assert_eq!(call(&setup.transport, LOGIN).status, 401);
    assert_eq!(call(&setup.transport, SETS).body, refused);
    assert!(
        !setup.transport.state().any_served(),
        "never served over an error"
    );
    let (_fake, transport) = offline(&setup);
    let miss = call(&transport, LOGIN);
    assert_eq!(miss, ServerResponse::no_response(OFFLINE_NO_CACHE_BODY));
    assert_eq!(call(&transport, SETS).body, OFFLINE_NO_CACHE_BODY);
    assert!(!transport.state().any_served());
}

#[test]
fn requests_without_a_hash_and_other_kinds_pass_through() {
    let setup = setup();
    call(
        &setup.transport,
        "r=achievementsets&u=Tester&t=tok123&g=1234",
    );
    call(&setup.transport, "r=gameid&m=00");
    assert!(!setup.data.path().join("achievements").exists());
    let (fake, transport) = offline(&setup);
    let by_id = call(&transport, "r=achievementsets&u=Tester&t=tok123&g=1234");
    assert_eq!(by_id.body, "offline", "no m=: the server's own answer");
    assert_eq!(fake.kinds(), ["achievementsets"]);
}

#[test]
fn cached_login_needs_the_same_token() {
    let setup = setup();
    online_run(&setup);
    let (_fake, transport) = offline(&setup);
    let other = call(&transport, "r=login2&u=tester&t=other");
    assert_eq!(other.body, OFFLINE_NO_CACHE_BODY);
    assert_eq!(call(&transport, "r=login2&u=TESTER&t=tok123").status, 200);
}

#[test]
fn cached_session_merges_this_users_pending_unlocks_once() {
    let setup = setup();
    online_run(&setup);
    let journal = PendingJournal::new(setup.data.path());
    let entry = |id, user: &str, hash: &str| PendingEntry {
        kind: PendingKind::Unlock,
        id,
        hardcore: false,
        game_hash: hash.into(),
        user: user.into(),
        at_unix: 77,
        score: None,
    };
    for e in [
        entry(1, "Tester", HASH),
        entry(2, "tester", &HASH.to_ascii_uppercase()),
        entry(3, "Other", HASH),
        entry(4, "Tester", "ffff"),
    ] {
        journal.write(&e).unwrap();
    }
    let (_fake, transport) = offline(&setup);
    let body = call(&transport, START).body;
    assert_eq!(session_ids(&body), [1, 2], "id 1 kept once; others ignored");
    assert!(body.contains("\"When\":77"));
}

#[test]
fn delivered_award_joins_the_cached_session() {
    let setup = setup();
    online_run(&setup);
    let award = format!("r=awardachievement&u=Tester&t=tok123&a=9&h=0&m={HASH}");
    assert_eq!(call(&setup.transport, &award).status, 200);
    let (_fake, transport) = offline(&setup);
    assert_eq!(session_ids(&call(&transport, START).body), [1, 9]);
}

#[test]
fn cached_unknown_game_is_served_without_the_sync_signal() {
    let setup = setup();
    setup
        .fake
        .respond("achievementsets", 200, FAKE_UNKNOWN_GAME_JSON);
    online_run(&setup);
    let (_fake, transport) = offline(&setup);
    assert_eq!(call(&transport, SETS).body, FAKE_UNKNOWN_GAME_JSON);
    let state = transport.state();
    assert!(
        state.any_served() && !state.set_served(),
        "GameId 0: no sync popup"
    );
}
