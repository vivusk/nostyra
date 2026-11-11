//! Nostyra netplay signaling server (M20): room codes → matchbox WebRTC signaling rooms,
//! HMAC room tokens, coturn REST credentials, per-IP rate limits and room GC. Runs on the
//! VPS behind nginx (`127.0.0.1:3536`); keeps nothing on disk.
//!
//! Logs never contain room codes, tokens, IPs, ICE candidates or TURN credentials: room
//! events carry an 8-hex hash of the room id only.
#![deny(unsafe_code)]

pub mod api_error;
pub mod app_state;
pub mod random_ids;
pub mod rate_limits;
pub mod request_validation;
pub mod room_code;
pub mod room_gc;
pub mod room_model;
mod room_presence;
pub mod room_registry;
pub mod room_token;
pub mod rooms_http_api;
pub mod seat_role;
pub mod server_config;
pub mod signaling_mesh;
mod signaling_peer_io;
pub mod signaling_ws;
pub mod turn_credentials;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_governor::GovernorLayer;

use crate::app_state::AppState;
use crate::rate_limits::{RateLimits, RouteLimit};
use crate::rooms_http_api::{create_room, join_room, room_status, start_room};
use crate::signaling_ws::signaling_ws;

/// Request bodies are a few hundred bytes of JSON.
const MAX_BODY_BYTES: usize = 4 * 1024;

/// The whole HTTP surface. Serve it with `into_make_service_with_connect_info::<SocketAddr>()`
/// (the rate limiter keys on the socket peer).
pub fn build_router(state: AppState, limits: &RateLimits) -> Router {
    let layer = |limit: &RouteLimit| GovernorLayer::new(limit.clone());
    Router::new()
        .route("/v1/rooms", post(create_room).layer(layer(&limits.create)))
        .route(
            "/v1/rooms/{code}",
            get(room_status).layer(layer(&limits.status)),
        )
        .route(
            "/v1/rooms/{code}/join",
            post(join_room).layer(layer(&limits.join)),
        )
        .route(
            "/v1/rooms/{code}/start",
            post(start_room).layer(layer(&limits.status)),
        )
        .route(
            "/v1/ws/{room_id}",
            get(signaling_ws).layer(layer(&limits.ws)),
        )
        .route("/healthz", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}
