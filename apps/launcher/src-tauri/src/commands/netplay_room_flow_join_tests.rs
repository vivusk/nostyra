//! Join / watch flow against the fake room API: `max_players` from the reply (F21),
//! no code in args, library lookup by content sha1, refusal texts.

use super::fake_api::*;
use super::*;

#[test]
fn join_uses_the_reply_cap_and_never_carries_the_code() {
    let api = FakeRoomApi::default();
    let (game_id, play) = open_join_seat(
        &api,
        false,
        &settings(),
        "k7q-2zp",
        SeatRole::Player,
        |sha1| {
            assert_eq!(sha1, "ab".repeat(20));
            Ok(Some(7))
        },
    )
    .unwrap();
    assert_eq!(game_id, 7);
    assert_eq!(api.joined.lock().unwrap()[0].0, "K7Q2ZP", "code normalised");
    let env = session_env(&play);
    assert_eq!(
        (env.code, env.max_players, env.role),
        (None, 4, SeatRole::Player)
    );
    let args = args_text(&play);
    assert!(
        args.starts_with("--netplay join --netplay-nickname Vivi"),
        "{args}"
    );
    assert!(
        !args.contains("K7Q2ZP") && !args.contains("--netplay-players"),
        "{args}"
    );
    let (_, watch) = open_join_seat(
        &api,
        false,
        &settings(),
        "K7Q2ZP",
        SeatRole::Spectator,
        |_| Ok(Some(7)),
    )
    .unwrap();
    assert!(args_text(&watch).starts_with("--netplay watch"));
}

#[test]
fn join_without_the_game_names_it_and_refusals_map() {
    let api = FakeRoomApi::default();
    let err = open_join_seat(&api, false, &settings(), "K7Q2ZP", SeatRole::Player, |_| {
        Ok(None)
    })
    .unwrap_err();
    assert_eq!(err.message, "You need Super Game (SNES) in your library");
    let started = FakeRoomApi {
        join_error: Some(NetplayError::RoomStarted),
        ..FakeRoomApi::default()
    };
    let err = open_join_seat(
        &started,
        false,
        &settings(),
        "K7Q2ZP",
        SeatRole::Player,
        |_| Ok(Some(1)),
    )
    .unwrap_err();
    assert_eq!(err.message, "Game already started");
    let bad = open_join_seat(&api, false, &settings(), "K7Q", SeatRole::Player, |_| {
        Ok(None)
    });
    assert!(bad.is_err());
}
