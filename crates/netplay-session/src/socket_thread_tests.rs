use super::*;
use crate::room_api_types::{IceServer, SeatRole};

fn env(ice: Vec<IceServer>) -> NetplaySessionEnv {
    NetplaySessionEnv {
        server_ws_url: "wss://x/v1/ws".into(),
        room_id: "r".into(),
        code: None,
        token: "t".into(),
        role: SeatRole::Player,
        max_players: 2,
        ice_servers: ice,
        expires_at: 0,
    }
}

#[test]
fn ice_list_merges_and_prefers_turn_on_request() {
    let stun = IceServer {
        urls: vec!["stun:h:3478".into()],
        username: None,
        credential: None,
    };
    let turn = IceServer {
        urls: vec![
            "turn:h:3478?transport=udp".into(),
            "turn:h:3478?transport=tcp".into(),
        ],
        username: Some("u".into()),
        credential: Some("c".into()),
    };
    let env = env(vec![stun, turn]);
    let plain = ice_config(&env, false);
    assert_eq!(plain.urls[0], "stun:h:3478");
    assert_eq!(
        (plain.username.as_deref(), plain.credential.as_deref()),
        (Some("u"), Some("c"))
    );
    let relay = ice_config(&env, true);
    assert_eq!(relay.urls.len(), 3);
    assert!(relay.urls[0].starts_with("turn:") && relay.urls[2].starts_with("stun:"));
}

#[test]
fn peer_ids_round_trip() {
    let id = PeerId(uuid::Uuid::from_u128(
        0x0123_4567_89ab_cdef_0011_2233_4455_6677,
    ));
    assert_eq!(peer_id(key(id)), id);
}
