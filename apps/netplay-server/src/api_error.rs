//! HTTP errors: one status + one stable `error` code per refusal (`{"error": "room_full"}`).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::room_model::RegistryError;

/// Every refusal the API answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    BadRequest(&'static str),
    MissingToken,
    BadToken,
    Registry(RegistryError),
}

impl ApiError {
    /// Status + code (the code is what the launcher maps to a message).
    pub fn parts(self) -> (StatusCode, &'static str) {
        use RegistryError as R;
        match self {
            Self::BadRequest(code) => (StatusCode::BAD_REQUEST, code),
            Self::MissingToken => (StatusCode::UNAUTHORIZED, "missing_token"),
            Self::BadToken => (StatusCode::FORBIDDEN, "bad_token"),
            Self::Registry(e) => match e {
                R::NotFound => (StatusCode::NOT_FOUND, "room_not_found"),
                R::Expired => (StatusCode::GONE, "room_expired"),
                R::RoomFull => (StatusCode::CONFLICT, "room_full"),
                R::Started => (StatusCode::CONFLICT, "room_started"),
                R::SpectatorsOff => (StatusCode::CONFLICT, "spectators_off"),
                R::AlreadyConnected => (StatusCode::CONFLICT, "already_connected"),
                R::NotHost => (StatusCode::FORBIDDEN, "not_host"),
                R::UnknownSeat => (StatusCode::FORBIDDEN, "bad_token"),
                R::TooManyRooms => (StatusCode::SERVICE_UNAVAILABLE, "server_full"),
                R::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
            },
        }
    }
}

impl From<RegistryError> for ApiError {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}
