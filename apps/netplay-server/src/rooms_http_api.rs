//! REST handlers under `/v1/rooms`: create, join, status poll, start.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api_error::ApiError;
use crate::app_state::{unix_now, AppState};
use crate::random_ids::log_hash;
use crate::request_validation::{clean_game_hint, clean_nickname};
use crate::room_code::normalize_room_code;
use crate::room_model::{GameHint, NewRoom, RegistryError, RoomStatus, RosterEntry, SeatGrant};
use crate::room_token::{sign_token, verify_token, TOKEN_TTL_SECS};
use crate::seat_role::SeatRole;
use crate::turn_credentials::{ice_servers, IceServer};

#[derive(Deserialize)]
pub struct CreateRoomBody {
    nickname: String,
    game: GameHint,
    max_players: u8,
    allow_spectators: bool,
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    code: String,
    room_id: String,
    token: String,
    ice_servers: Vec<IceServer>,
    /// Token expiry (Unix seconds): the runner must open the WebSocket before it.
    expires_at: u64,
}

#[derive(Deserialize)]
pub struct JoinBody {
    nickname: String,
    role: SeatRole,
}

#[derive(Serialize)]
pub struct JoinResponse {
    room_id: String,
    token: String,
    ice_servers: Vec<IceServer>,
    roster: Vec<RosterEntry>,
    game: GameHint,
    max_players: u8,
    expires_at: u64,
}

/// `POST /v1/rooms` (host).
pub async fn create_room(
    State(app): State<AppState>,
    Json(body): Json<CreateRoomBody>,
) -> Result<Json<CreateRoomResponse>, ApiError> {
    if !(2..=4).contains(&body.max_players) {
        return Err(ApiError::BadRequest("bad_max_players"));
    }
    let req = NewRoom {
        nickname: clean_nickname(&body.nickname)?,
        game: clean_game_hint(body.game)?,
        max_players: body.max_players,
        allow_spectators: body.allow_spectators,
    };
    let grant = app.registry.create(req, Instant::now())?;
    tracing::info!(room = %log_hash(&grant.room_id), max_players = grant.max_players, "room created");
    let (token, ice_servers, expires_at) = credentials(&app, &grant);
    Ok(Json(CreateRoomResponse {
        code: grant.code,
        room_id: grant.room_id,
        token,
        ice_servers,
        expires_at,
    }))
}

/// `POST /v1/rooms/{code}/join` (player / spectator).
pub async fn join_room(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Json(body): Json<JoinBody>,
) -> Result<Json<JoinResponse>, ApiError> {
    let code = normalize_room_code(&code).ok_or(RegistryError::NotFound)?;
    if body.role == SeatRole::Host {
        return Err(ApiError::BadRequest("bad_role"));
    }
    let nickname = clean_nickname(&body.nickname)?;
    let grant = app
        .registry
        .join(&code, nickname, body.role, Instant::now())?;
    tracing::info!(room = %log_hash(&grant.room_id), role = grant.role.as_str(), "seat issued");
    let (token, ice_servers, expires_at) = credentials(&app, &grant);
    Ok(Json(JoinResponse {
        room_id: grant.room_id,
        token,
        ice_servers,
        roster: grant.roster,
        game: grant.game,
        max_players: grant.max_players,
        expires_at,
    }))
}

/// `GET /v1/rooms/{code}` (lobby poll).
pub async fn room_status(
    State(app): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomStatus>, ApiError> {
    let code = normalize_room_code(&code).ok_or(RegistryError::NotFound)?;
    Ok(Json(app.registry.status(&code, Instant::now())?))
}

/// `POST /v1/rooms/{code}/start` with `Authorization: Bearer <host token>` → 204.
pub async fn start_room(
    State(app): State<AppState>,
    Path(code): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let code = normalize_room_code(&code).ok_or(RegistryError::NotFound)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::MissingToken)?;
    let claims = verify_token(&app.config.server_secret, token.trim(), unix_now())
        .map_err(|_| ApiError::BadToken)?;
    app.registry.start(&code, &claims.room_id, claims.role)?;
    tracing::info!(room = %log_hash(&claims.room_id), "room started");
    Ok(StatusCode::NO_CONTENT)
}

/// Token + ICE list for a fresh seat.
fn credentials(app: &AppState, grant: &SeatGrant) -> (String, Vec<IceServer>, u64) {
    let now = unix_now();
    let cfg = &app.config;
    let token = sign_token(
        &cfg.server_secret,
        &grant.room_id,
        &grant.peer_id,
        grant.role,
        now,
    );
    let ice = ice_servers(&cfg.turn_host, &cfg.turn_secret, &grant.peer_id, now);
    (token, ice, now + TOKEN_TTL_SECS)
}
