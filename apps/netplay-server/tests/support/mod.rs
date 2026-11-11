//! Shared helpers: an in-process router (`oneshot`) and a real server on 127.0.0.1:0.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use netplay_server::app_state::AppState;
use netplay_server::rate_limits::RateLimits;
use netplay_server::server_config::ServerConfig;
use serde_json::{json, Value};
use tokio::sync::watch;
use tower::ServiceExt;

pub const SECRET: &str = "integration-test-secret-0123456789abcdef";

pub fn test_config() -> ServerConfig {
    ServerConfig::from_lookup(|name| match name {
        "NETPLAY_SERVER_SECRET" => Some(SECRET.into()),
        "TURN_SECRET" => Some("turn-secret-for-tests".into()),
        "TURN_HOST" => Some("turn.test".into()),
        _ => None,
    })
    .unwrap()
}

/// Router + its state; keep the sender alive for the test's duration.
pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub stop: watch::Sender<bool>,
}

pub fn test_app() -> TestApp {
    let config = test_config();
    let limits = RateLimits::new(&config.trusted_proxies);
    let (stop, stop_rx) = watch::channel(false);
    let state = AppState::new(config, stop_rx);
    let router = netplay_server::build_router(state.clone(), &limits);
    TestApp {
        router,
        state,
        stop,
    }
}

/// One request through the router as if it came from socket peer `from`.
pub async fn call(
    app: &TestApp,
    method: &str,
    uri: &str,
    body: Option<Value>,
    from: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let mut req = builder.body(body).unwrap();
    let peer: SocketAddr = from.parse().unwrap();
    req.extensions_mut().insert(ConnectInfo(peer));
    let resp = app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, value)
}

pub const CLIENT: &str = "198.51.100.1:5000";

pub fn create_body(max_players: u8, allow_spectators: bool) -> Value {
    json!({
        "nickname": "Host",
        "game": {
            "content_sha1": "a".repeat(40),
            "title": "Test Game",
            "system": "snes",
            "core_id": "snes9x",
            "core_version": "1.62"
        },
        "max_players": max_players,
        "allow_spectators": allow_spectators
    })
}

pub async fn create_room(app: &TestApp, max_players: u8, spectators: bool) -> Value {
    let (status, body) = call(
        app,
        "POST",
        "/v1/rooms",
        Some(create_body(max_players, spectators)),
        CLIENT,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub async fn join(app: &TestApp, code: &str, nick: &str, role: &str) -> (StatusCode, Value) {
    let uri = format!("/v1/rooms/{code}/join");
    let body = json!({ "nickname": nick, "role": role });
    call(app, "POST", &uri, Some(body), CLIENT, &[]).await
}

/// Serves `app` on an ephemeral loopback port.
pub async fn serve(app: &TestApp) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = app
        .router
        .clone()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
    addr
}
