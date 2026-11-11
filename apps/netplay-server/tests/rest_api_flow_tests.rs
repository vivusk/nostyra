//! Full REST flow through the router (no network).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::*;

#[tokio::test]
async fn create_join_full_start_late_join() {
    let app = test_app();
    let host = create_room(&app, 4, true).await;
    let code = host["code"].as_str().unwrap();
    assert_eq!(code.len(), 6);
    assert_eq!(host["room_id"].as_str().unwrap().len(), 32);
    assert!(host["expires_at"].as_u64().unwrap() > 0);
    let ice = host["ice_servers"].as_array().unwrap();
    assert_eq!(ice[0]["urls"][0], "stun:turn.test:3478");
    assert_eq!(ice[1]["urls"][1], "turn:turn.test:3478?transport=tcp");
    assert!(ice[1]["credential"].as_str().is_some());

    for i in 0..3 {
        // Lower case + look-alikes are normalised.
        let typed = code.to_lowercase().replace('0', "o");
        let (status, body) = join(&app, &typed, &format!("P{i}"), "player").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["room_id"], host["room_id"]);
        assert_eq!(body["max_players"], 4, "F21: the joiner learns max_players");
        assert_eq!(body["game"]["core_id"], "snes9x");
        assert_eq!(body["roster"].as_array().unwrap().len(), i + 2);
        assert!(body["token"].as_str().is_some());
    }
    let (status, body) = join(&app, code, "P4", "player").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("room_full"))
    );

    let (status, body) = call(&app, "GET", &format!("/v1/rooms/{code}"), None, CLIENT, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"players": 0, "spectators": 0, "started": false, "max_players": 4})
    );

    let start = format!("/v1/rooms/{code}/start");
    let (_, guest) = join(&app, code, "Watcher", "spectator").await;
    let guest_auth = format!("Bearer {}", guest["token"].as_str().unwrap());
    let (status, _) = call(
        &app,
        "POST",
        &start,
        None,
        CLIENT,
        &[("authorization", &guest_auth)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "non-host token (F22)");
    let (status, _) = call(&app, "POST", &start, None, CLIENT, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let host_auth = format!("Bearer {}", host["token"].as_str().unwrap());
    let (status, _) = call(
        &app,
        "POST",
        &start,
        None,
        CLIENT,
        &[("authorization", &host_auth)],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = join(&app, code, "Late", "spectator").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("room_started"))
    );
    let (_, body) = call(&app, "GET", &format!("/v1/rooms/{code}"), None, CLIENT, &[]).await;
    assert_eq!(body["started"], true);
}

#[tokio::test]
async fn host_token_of_another_room_cannot_start() {
    let app = test_app();
    let a = create_room(&app, 2, false).await;
    let b = create_room(&app, 2, false).await;
    let uri = format!("/v1/rooms/{}/start", a["code"].as_str().unwrap());
    let auth = format!("Bearer {}", b["token"].as_str().unwrap());
    let (status, _) = call(
        &app,
        "POST",
        &uri,
        None,
        CLIENT,
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn refusals_have_their_statuses() {
    let app = test_app();
    let (status, _) = join(&app, "ZZZZZZ", "P", "player").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = join(&app, "not-a-code", "P", "player").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let room = create_room(&app, 2, false).await;
    let code = room["code"].as_str().unwrap();
    let (status, body) = join(&app, code, "S", "spectator").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("spectators_off"))
    );
    let (status, _) = join(&app, code, "H", "host").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = join(&app, code, "  ", "player").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for players in [1, 5] {
        let body = Some(create_body(players, false));
        let (status, _) = call(&app, "POST", "/v1/rooms", body, CLIENT, &[]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let huge = Some(json!({ "nickname": "x".repeat(8000) }));
    let (status, _) = call(&app, "POST", "/v1/rooms", huge, CLIENT, &[]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn health() {
    let app = test_app();
    let (status, body) = call(&app, "GET", "/healthz", None, CLIENT, &[]).await;
    assert_eq!((status, body), (StatusCode::OK, json!("ok")));
}
