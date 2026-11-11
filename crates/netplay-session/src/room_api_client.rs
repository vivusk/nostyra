//! Blocking REST client for the signaling server's `/v1/rooms*` (feature `rest`,
//! launcher only): create, join, status, start (host token, F22). 10 s timeouts,
//! `NOSTYRA_OFFLINE=1` → [`NetplayError::Offline`] before any socket, statuses and
//! the server's `{"error": code}` mapped to [`NetplayError`]s. URLs, codes and
//! tokens never reach an error text or a log line.

use std::io::Read;
use std::time::Duration;

use http_download_util::{
    build_http_client, HttpClient, HttpMethod, HttpReply, HttpRequest, RequestBody,
};
use serde::de::DeserializeOwned;

use crate::room_api_rules::{clean_room_code, status_error, ServerUrls};
use crate::room_api_types::{
    CreateRoomRequest, CreateRoomResponse, JoinRequest, JoinResponse, RoomStatus,
};
use crate::NetplayError;

/// Per-request bound (connect and whole call).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REPLY_BYTES: u64 = 64 * 1024;

/// The HTTP seam (live: the shared `HttpClient`; tests: a scripted fake).
pub trait RoomTransport: Send + Sync {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, NetplayError>;
}

impl RoomTransport for HttpClient {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, NetplayError> {
        // The transport error text may contain the url (room code): dropped.
        self.request(request).map_err(|_| NetplayError::Network)
    }
}

/// True when `NOSTYRA_OFFLINE=1`.
pub fn offline_from_env() -> bool {
    std::env::var("NOSTYRA_OFFLINE").is_ok_and(|value| value == "1")
}

/// Room API client.
pub struct RoomApiClient {
    transport: Box<dyn RoomTransport>,
    rest_base: String,
    offline: bool,
}

impl RoomApiClient {
    pub fn new(urls: &ServerUrls, transport: Box<dyn RoomTransport>, offline: bool) -> Self {
        Self {
            transport,
            rest_base: urls.rest_base.clone(),
            offline,
        }
    }

    /// Live client on the shared HTTPS client; honours `NOSTYRA_OFFLINE`.
    pub fn live(urls: &ServerUrls, user_agent: &str) -> Self {
        let offline = offline_from_env();
        Self::new(
            urls,
            Box::new(build_http_client(user_agent, offline)),
            offline,
        )
    }

    /// `POST /v1/rooms` (host).
    pub fn create(&self, body: &CreateRoomRequest) -> Result<CreateRoomResponse, NetplayError> {
        let json = serde_json::to_vec(body).map_err(|_| NetplayError::BadReply)?;
        parse(self.call(HttpMethod::Post, "/v1/rooms", Some(&json), None)?)
    }

    /// `POST /v1/rooms/{code}/join` (player / spectator).
    pub fn join(&self, code: &str, body: &JoinRequest) -> Result<JoinResponse, NetplayError> {
        let path = format!("/v1/rooms/{}/join", clean_room_code(code)?);
        let json = serde_json::to_vec(body).map_err(|_| NetplayError::BadReply)?;
        parse(self.call(HttpMethod::Post, &path, Some(&json), None)?)
    }

    /// `GET /v1/rooms/{code}` (lobby poll).
    pub fn status(&self, code: &str) -> Result<RoomStatus, NetplayError> {
        let path = format!("/v1/rooms/{}", clean_room_code(code)?);
        parse(self.call(HttpMethod::Get, &path, None, None)?)
    }

    /// `POST /v1/rooms/{code}/start` with the host token (on the runner's `Started`).
    pub fn start(&self, code: &str, host_token: &str) -> Result<(), NetplayError> {
        let path = format!("/v1/rooms/{}/start", clean_room_code(code)?);
        self.call(HttpMethod::Post, &path, None, Some(host_token))
            .map(drop)
    }

    /// One request; the 2xx body (≤ 64 KiB) or the mapped error.
    fn call(
        &self,
        method: HttpMethod,
        path: &str,
        json: Option<&[u8]>,
        bearer: Option<&str>,
    ) -> Result<Vec<u8>, NetplayError> {
        if self.offline {
            return Err(NetplayError::Offline);
        }
        let url = format!("{}{path}", self.rest_base);
        let auth = bearer.map(|token| format!("Bearer {token}"));
        let mut headers = vec![("Accept", "application/json")];
        if json.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        if let Some(auth) = auth.as_deref() {
            headers.push(("Authorization", auth));
        }
        let reply = self.transport.send(HttpRequest {
            method,
            url: &url,
            headers: &headers,
            body: json.map_or(RequestBody::Empty, RequestBody::Bytes),
            connect_timeout: Some(REQUEST_TIMEOUT),
            timeout: Some(REQUEST_TIMEOUT),
        })?;
        let status = reply.status;
        let retry_after = reply
            .header("retry-after")
            .and_then(|v| v.trim().parse::<u64>().ok());
        let body = read_capped(reply)?;
        if (200..300).contains(&status) {
            return Ok(body);
        }
        Err(status_error(status, &body, retry_after))
    }
}

fn read_capped(reply: HttpReply) -> Result<Vec<u8>, NetplayError> {
    let mut body = Vec::new();
    reply
        .body
        .take(MAX_REPLY_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|_| NetplayError::Network)?;
    if body.len() as u64 > MAX_REPLY_BYTES {
        return Err(NetplayError::BadReply);
    }
    Ok(body)
}

fn parse<T: DeserializeOwned>(body: Vec<u8>) -> Result<T, NetplayError> {
    serde_json::from_slice(&body).map_err(|_| NetplayError::BadReply)
}

#[cfg(test)]
#[path = "room_api_client_tests.rs"]
mod tests;
