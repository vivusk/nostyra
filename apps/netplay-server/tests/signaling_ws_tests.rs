//! WebSocket upgrade checks (router, no network) and the matchbox mesh on a real
//! loopback server with tungstenite clients.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::time::Duration;

use axum::http::StatusCode;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use support::*;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

type Client =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::test]
async fn upgrade_without_or_with_a_wrong_token_is_refused() {
    let app = test_app();
    let a = create_room(&app, 2, false).await;
    let b = create_room(&app, 2, false).await;
    let room = a["room_id"].as_str().unwrap();
    let (status, _) = call(&app, "GET", &format!("/v1/ws/{room}"), None, CLIENT, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let bad = format!("/v1/ws/{room}?token=bogus.token");
    assert_eq!(
        call(&app, "GET", &bad, None, CLIENT, &[]).await.0,
        StatusCode::FORBIDDEN
    );
    let other = format!("/v1/ws/{room}?token={}", b["token"].as_str().unwrap());
    assert_eq!(
        call(&app, "GET", &other, None, CLIENT, &[]).await.0,
        StatusCode::FORBIDDEN
    );
}

async fn connect(addr: std::net::SocketAddr, room: &Value) -> Result<Client, WsError> {
    let url = format!(
        "ws://{addr}/v1/ws/{}?token={}",
        room["room_id"].as_str().unwrap(),
        room["token"].as_str().unwrap()
    );
    tokio_tungstenite::connect_async(url)
        .await
        .map(|(ws, _)| ws)
}

async fn next_event(ws: &mut Client) -> Value {
    let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("event in time")
        .expect("socket open")
        .expect("no ws error");
    serde_json::from_str(msg.to_text().unwrap()).unwrap()
}

fn refused_status(result: Result<Client, WsError>) -> u16 {
    match result {
        Err(WsError::Http(resp)) => resp.status().as_u16(),
        Err(e) => panic!("unexpected error {e}"),
        Ok(_) => panic!("upgrade should be refused"),
    }
}

#[tokio::test]
async fn mesh_announces_relays_and_counts_live_peers() {
    let app = test_app();
    let addr = serve(&app).await;
    let host = create_room(&app, 2, false).await;
    let code = host["code"].as_str().unwrap();
    let (_, guest) = join(&app, code, "Guest", "player").await;
    let guest = json!({ "room_id": guest["room_id"], "token": guest["token"] });

    let mut host_ws = connect(addr, &host).await.unwrap();
    let host_id = next_event(&mut host_ws).await["IdAssigned"].clone();
    let mut guest_ws = connect(addr, &guest).await.unwrap();
    let guest_id = next_event(&mut guest_ws).await["IdAssigned"].clone();
    assert!(host_id.is_string() && guest_id.is_string());
    assert_eq!(
        next_event(&mut host_ws).await,
        json!({ "NewPeer": guest_id })
    );

    let signal = json!({ "Signal": { "receiver": host_id, "data": { "sdp": "x" } } });
    guest_ws
        .send(Message::text(signal.to_string()))
        .await
        .unwrap();
    let relayed = next_event(&mut host_ws).await;
    assert_eq!(
        relayed,
        json!({ "Signal": { "sender": guest_id, "data": { "sdp": "x" } } })
    );
    guest_ws.send(Message::text("\"KeepAlive\"")).await.unwrap();

    let (_, status) = call(&app, "GET", &format!("/v1/rooms/{code}"), None, CLIENT, &[]).await;
    assert_eq!(status["players"], 2, "F28: counted from live connections");

    // Same token twice → 409; the live cap holds.
    assert_eq!(refused_status(connect(addr, &guest).await), 409);

    guest_ws.close(None).await.unwrap();
    assert_eq!(
        next_event(&mut host_ws).await,
        json!({ "PeerLeft": guest_id })
    );
    let room_id = host["room_id"].as_str().unwrap();
    for _ in 0..50 {
        let (_, status) = call(&app, "GET", &format!("/v1/rooms/{code}"), None, CLIENT, &[]).await;
        if status["players"] == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(app.state.mesh.peer_count(room_id), 1);
}

#[tokio::test]
async fn unknown_room_and_shutdown_close() {
    let app = test_app();
    let addr = serve(&app).await;
    let host = create_room(&app, 2, false).await;
    let bad_room = json!({ "room_id": "0".repeat(32), "token": host["token"] });
    assert_eq!(refused_status(connect(addr, &bad_room).await), 403);
    let mut ws = connect(addr, &host).await.unwrap();
    next_event(&mut ws).await;
    app.stop.send_replace(true);
    let closed = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .unwrap();
    assert!(
        matches!(closed, Some(Ok(Message::Close(_))) | None),
        "{closed:?}"
    );
}

#[tokio::test]
async fn flooding_peer_is_closed_with_policy_violation() {
    let app = test_app();
    let addr = serve(&app).await;
    let host = create_room(&app, 2, false).await;
    let mut ws = connect(addr, &host).await.unwrap();
    next_event(&mut ws).await;
    for _ in 0..400 {
        if ws.feed(Message::text("\"KeepAlive\"")).await.is_err() {
            break;
        }
    }
    let _ = ws.flush().await;
    let code = loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Ok(Some(Ok(Message::Close(frame)))) => break frame.map(|f| u16::from(f.code)),
            Ok(Some(Ok(_))) => continue,
            other => panic!("expected a close frame, got {other:?}"),
        }
    };
    assert_eq!(code, Some(1008));
}
