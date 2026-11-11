//! Host flow against the fake room API: create body, runner args / env (no secret
//! in args or Debug), offline refusal, `/start` marking (F22).

use ipc_protocol::NetplayEvent;
use runner_session::RunnerLaunch;

use super::fake_api::*;
use super::*;

#[test]
fn host_creates_the_room_with_the_game_hint_and_builds_args_and_env() {
    let api = Arc::new(FakeRoomApi::default());
    let (code, play) = host(&api);
    assert_eq!(code, "K7Q2ZP");
    let created = api.created.lock().unwrap();
    assert_eq!(created[0].game, hint());
    assert_eq!(
        (created[0].max_players, created[0].nickname.as_str()),
        (2, "Vivi")
    );
    let args = args_text(&play);
    assert!(
        args.starts_with("--netplay host --netplay-players 2"),
        "{args}"
    );
    for secret in ["K7Q2ZP", TOKEN, "room1", "wss://", CREDENTIAL] {
        assert!(!args.contains(secret), "{secret} in args: {args}");
    }
    let env = session_env(&play);
    assert_eq!(play.env().0, "NOSTYRA_NETPLAY_SESSION");
    assert_eq!((env.room_id.as_str(), env.token.as_str()), ("room1", TOKEN));
    assert_eq!(env.code.as_deref(), Some("K7Q2ZP"));
    assert_eq!((env.role, env.max_players), (SeatRole::Host, 2));
    assert_eq!(env.ice_servers, ice(), "TURN credential intact");
    assert_eq!(env.server_ws_url, "wss://netplay.nostyra.com/v1/ws");
}

#[test]
fn offline_refuses_before_any_request() {
    let api = Arc::new(FakeRoomApi::default());
    let room = HostRoom {
        game: hint(),
        players: 2,
        allow_spectators: false,
    };
    let err = open_host_room(api.clone(), true, &settings(), room).unwrap_err();
    assert!(err.message.contains("offline"), "{}", err.message);
    let join = open_join_seat(
        api.as_ref(),
        true,
        &settings(),
        "K7Q2ZP",
        SeatRole::Player,
        |_| Ok(Some(1)),
    );
    assert!(join.is_err());
    assert!(api.created.lock().unwrap().is_empty() && api.joined.lock().unwrap().is_empty());
}

#[test]
fn host_marks_the_room_started_exactly_once() {
    let api = Arc::new(FakeRoomApi::default());
    let (_, mut play) = host(&api);
    let started = SessionUpdate::Netplay(NetplayEvent::Started {
        players: 2,
        input_delay: 1,
    });
    let marker = play.start.as_mut().unwrap();
    marker.on_update(&SessionUpdate::Netplay(NetplayEvent::Syncing {
        percent: 50,
    }));
    marker.on_update(&started);
    marker.on_update(&started);
    assert_eq!(
        *api.started.lock().unwrap(),
        vec![("K7Q2ZP".to_string(), TOKEN.to_string())]
    );
    let join = FakeRoomApi::default();
    let (_, guest) = open_join_seat(
        &join,
        false,
        &settings(),
        "K7Q2ZP",
        SeatRole::Player,
        |_| Ok(Some(1)),
    )
    .unwrap();
    assert!(guest.start.is_none(), "guests never call /start");
}

#[test]
fn debug_output_hides_token_code_and_credential() {
    let api = Arc::new(FakeRoomApi::default());
    let (_, play) = host(&api);
    let launch = RunnerLaunch {
        runner: "runner".into(),
        core: "core".into(),
        rom: "rom".into(),
        system: "snes".into(),
        data_dir: "data".into(),
        extra_args: play.args.clone(),
        env: vec![play.env()],
    };
    for text in [format!("{launch:?}"), format!("{play:?}")] {
        for secret in [TOKEN, CREDENTIAL, "K7Q2ZP", "1700:peer"] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
    }
}
