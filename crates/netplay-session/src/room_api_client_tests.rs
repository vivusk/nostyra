#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::*;
use crate::room_api_types::{GameHint, SeatRole};

/// Scripted transport (no network): queued replies, recorded requests.
type Reply = (u16, Vec<(String, String)>, String);
type Seen = (HttpMethod, String, Vec<(String, String)>, Vec<u8>);

#[derive(Default, Clone)]
struct FakeHttpTransport {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl FakeHttpTransport {
    fn push(&self, status: u16, body: &str) {
        self.replies
            .lock()
            .unwrap()
            .push_back((status, vec![], body.to_owned()));
    }

    fn push_with_header(&self, status: u16, header: (&str, &str)) {
        let headers = vec![(header.0.to_owned(), header.1.to_owned())];
        self.replies
            .lock()
            .unwrap()
            .push_back((status, headers, String::new()));
    }
}

impl RoomTransport for FakeHttpTransport {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, NetplayError> {
        let body = match request.body {
            RequestBody::Bytes(bytes) => bytes.to_vec(),
            _ => Vec::new(),
        };
        let headers = request
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.seen
            .lock()
            .unwrap()
            .push((request.method, request.url.to_owned(), headers, body));
        let (status, headers, body) = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(NetplayError::Network)?;
        Ok(HttpReply {
            status,
            headers,
            body: Box::new(std::io::Cursor::new(body.into_bytes())),
        })
    }
}

fn client(offline: bool) -> (RoomApiClient, FakeHttpTransport) {
    let fake = FakeHttpTransport::default();
    let urls = ServerUrls::parse("wss://netplay.example/").unwrap();
    (
        RoomApiClient::new(&urls, Box::new(fake.clone()), offline),
        fake,
    )
}

fn game() -> GameHint {
    GameHint {
        content_sha1: "ab".repeat(20),
        title: "Game".into(),
        system: "snes".into(),
        core_id: "snes9x".into(),
        core_version: "1.62".into(),
    }
}

#[test]
fn create_posts_json_and_parses_the_reply() {
    let (client, fake) = client(false);
    fake.push(200, r#"{"code":"K7Q2ZP","room_id":"r1","token":"t.s","ice_servers":[{"urls":["stun:h:3478"]}],"expires_at":5}"#);
    let request = CreateRoomRequest {
        nickname: "Ann".into(),
        game: game(),
        max_players: 4,
        allow_spectators: true,
    };
    let reply = client.create(&request).unwrap();
    assert_eq!((reply.code.as_str(), reply.expires_at), ("K7Q2ZP", 5));
    let seen = fake.seen.lock().unwrap();
    assert_eq!(seen[0].0, HttpMethod::Post);
    assert_eq!(seen[0].1, "https://netplay.example/v1/rooms");
    let body: serde_json::Value = serde_json::from_slice(&seen[0].3).unwrap();
    assert_eq!(body["max_players"], 4);
}

#[test]
fn join_parses_max_players_and_cleans_the_code() {
    let (client, fake) = client(false);
    fake.push(
        200,
        r#"{"room_id":"r1","token":"t","ice_servers":[],"roster":[],
        "game":{"content_sha1":"x","title":"T","system":"s","core_id":"c","core_version":"1"},
        "max_players":3,"expires_at":9}"#,
    );
    let request = JoinRequest {
        nickname: "Bob".into(),
        role: SeatRole::Spectator,
    };
    let reply = client.join("k7q-2zp", &request).unwrap();
    assert_eq!(reply.max_players, 3);
    let seen = fake.seen.lock().unwrap();
    assert_eq!(seen[0].1, "https://netplay.example/v1/rooms/K7Q2ZP/join");
    assert!(String::from_utf8_lossy(&seen[0].3).contains("\"spectator\""));
    assert_eq!(
        client.join("../x/y", &request).unwrap_err(),
        NetplayError::BadRoomCode
    );
}

#[test]
fn start_sends_the_bearer_and_maps_204_and_403() {
    let (client, fake) = client(false);
    fake.push(204, "");
    fake.push(403, r#"{"error":"not_host"}"#);
    client.start("K7Q2ZP", "host.token").unwrap();
    assert_eq!(
        client.start("K7Q2ZP", "guest.token").unwrap_err(),
        NetplayError::Forbidden
    );
    let seen = fake.seen.lock().unwrap();
    let auth = seen[0]
        .2
        .iter()
        .find(|(k, _)| k == "Authorization")
        .unwrap();
    assert_eq!(auth.1, "Bearer host.token");
    assert_eq!(seen[0].1, "https://netplay.example/v1/rooms/K7Q2ZP/start");
}

#[test]
fn error_statuses_map_to_errors() {
    let (client, fake) = client(false);
    let request = JoinRequest {
        nickname: "Bob".into(),
        role: SeatRole::Player,
    };
    let cases: [(u16, &str, NetplayError); 6] = [
        (
            404,
            r#"{"error":"room_not_found"}"#,
            NetplayError::RoomNotFound,
        ),
        (409, r#"{"error":"room_full"}"#, NetplayError::RoomFull),
        (
            409,
            r#"{"error":"room_started"}"#,
            NetplayError::RoomStarted,
        ),
        (410, "gone", NetplayError::RoomExpired),
        (
            400,
            r#"{"error":"bad_nickname"}"#,
            NetplayError::BadRequest("bad_nickname".into()),
        ),
        (500, "oops", NetplayError::Server(500)),
    ];
    for (status, body, expected) in cases {
        fake.push(status, body);
        assert_eq!(
            client.join("K7Q2ZP", &request).unwrap_err(),
            expected,
            "{status}"
        );
    }
    fake.push_with_header(429, ("Retry-After", "12"));
    assert_eq!(
        client.status("K7Q2ZP").unwrap_err(),
        NetplayError::RateLimited {
            retry_after_secs: Some(12)
        }
    );
    fake.push(200, "not json");
    assert_eq!(client.status("K7Q2ZP").unwrap_err(), NetplayError::BadReply);
}

#[test]
fn offline_switch_fails_before_any_request() {
    let (client, fake) = client(true);
    assert_eq!(client.status("K7Q2ZP").unwrap_err(), NetplayError::Offline);
    assert!(fake.seen.lock().unwrap().is_empty());
}
