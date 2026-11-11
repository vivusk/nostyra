//! `GET /v1/ws/{room_id}?token=…`: verifies the token, maps its seat to a fresh matchbox
//! `PeerId` (live caps checked here, F28), then hands the socket to the room mesh.
//! Checks run before the upgrade, so refusals are plain HTTP statuses (401 / 403 / 404 /
//! 409 / 410) the client sees in the handshake.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use matchbox_protocol::PeerId;

use crate::api_error::ApiError;
use crate::app_state::{unix_now, AppState};
use crate::random_ids::log_hash;
use crate::room_registry::RoomRegistry;
use crate::room_token::verify_token;

/// Signaling messages are small SDP / ICE JSON; cap what one peer can push.
const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// The upgrade handler.
pub async fn signaling_ws(
    State(app): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    match admit(&app, &room_id, query.get("token"), upgrade) {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

fn admit(
    app: &AppState,
    room_id: &str,
    token: Option<&String>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, ApiError> {
    let token = token.ok_or(ApiError::MissingToken)?;
    let claims = verify_token(&app.config.server_secret, token, unix_now())
        .map_err(|_| ApiError::BadToken)?;
    if claims.room_id != room_id {
        return Err(ApiError::BadToken);
    }
    let upgrade = match upgrade {
        Ok(upgrade) => upgrade,
        Err(rejection) => return Ok(rejection.into_response()),
    };
    let peer = PeerId(uuid::Uuid::new_v4());
    let role = app
        .registry
        .connect(room_id, &claims.peer_id, peer, Instant::now())?;
    let room_hash = log_hash(room_id);
    tracing::info!(room = %room_hash, role = role.as_str(), "peer connected");
    // Released when the socket task ends, or when the upgrade never completes and the
    // callback is dropped unrun.
    let seat = SeatRelease {
        registry: Arc::clone(&app.registry),
        room_id: room_id.to_owned(),
        peer_id: claims.peer_id,
        room_hash,
    };
    let (mesh, shutdown, room) = (app.mesh.clone(), app.shutdown.clone(), room_id.to_owned());
    let response = upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let _seat = seat;
            mesh.run_peer(socket, room, peer, shutdown).await;
        });
    Ok(response)
}

/// Frees the seat's live slot on drop.
struct SeatRelease {
    registry: Arc<RoomRegistry>,
    room_id: String,
    peer_id: String,
    room_hash: String,
}

impl Drop for SeatRelease {
    fn drop(&mut self) {
        self.registry
            .disconnect(&self.room_id, &self.peer_id, Instant::now());
        tracing::info!(room = %self.room_hash, "peer disconnected");
    }
}
