//! Pure rules of the room API (no HTTP; also usable without `rest`): server url →
//! REST / WebSocket bases, room code clean-up for the url path, and the mapping of
//! the server's statuses + `{"error": code}` bodies to [`NetplayError`]s.

use crate::NetplayError;

/// REST base and WebSocket base derived from the configured server url
/// (`wss://host` or `https://host`; `ws://` / `http://` for local tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerUrls {
    pub rest_base: String,
    /// `…/v1/ws` (`NetplaySessionEnv::server_ws_url`).
    pub ws_base: String,
}

impl ServerUrls {
    pub fn parse(setting: &str) -> Result<Self, NetplayError> {
        let setting = setting.trim().trim_end_matches('/');
        let (secure, host) = [
            ("wss://", true),
            ("https://", true),
            ("ws://", false),
            ("http://", false),
        ]
        .iter()
        .find_map(|(scheme, secure)| setting.strip_prefix(scheme).map(|rest| (*secure, rest)))
        .ok_or(NetplayError::BadServerUrl)?;
        if host.is_empty() || host.contains(['?', '#', ' ']) {
            return Err(NetplayError::BadServerUrl);
        }
        let (http, ws) = if secure {
            ("https", "wss")
        } else {
            ("http", "ws")
        };
        Ok(Self {
            rest_base: format!("{http}://{host}"),
            ws_base: format!("{ws}://{host}/v1/ws"),
        })
    }
}

/// Normalised room code for the url path: spaces / dashes dropped, uppercased,
/// 6 ASCII alphanumerics (the server maps look-alikes).
pub fn clean_room_code(input: &str) -> Result<String, NetplayError> {
    let code: String = input
        .chars()
        .filter(|c| !matches!(c, ' ' | '-'))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(NetplayError::BadRoomCode);
    }
    Ok(code)
}

/// Status + `{"error": code}` → error (plain-text bodies map by status alone).
pub fn status_error(status: u16, body: &[u8], retry_after_secs: Option<u64>) -> NetplayError {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: String,
    }
    let code = serde_json::from_slice::<ErrorBody>(body)
        .map(|b| b.error)
        .unwrap_or_default();
    match (status, code.as_str()) {
        (404, _) => NetplayError::RoomNotFound,
        (410, _) => NetplayError::RoomExpired,
        (409, "room_started") => NetplayError::RoomStarted,
        (409, "spectators_off") => NetplayError::SpectatorsOff,
        (409, "already_connected") => NetplayError::AlreadyConnected,
        (409, _) => NetplayError::RoomFull,
        (401 | 403, _) => NetplayError::Forbidden,
        (429, _) => NetplayError::RateLimited { retry_after_secs },
        (503, "server_full") => NetplayError::ServerFull,
        (400, code) => NetplayError::BadRequest(
            code.chars()
                .filter(char::is_ascii_graphic)
                .take(40)
                .collect(),
        ),
        (status, _) => NetplayError::Server(status),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn server_urls_from_the_setting() {
        let urls = ServerUrls::parse("wss://netplay.example").unwrap();
        assert_eq!(urls.rest_base, "https://netplay.example");
        assert_eq!(urls.ws_base, "wss://netplay.example/v1/ws");
        let local = ServerUrls::parse("http://127.0.0.1:3536/").unwrap();
        assert_eq!(local.ws_base, "ws://127.0.0.1:3536/v1/ws");
        assert!(ServerUrls::parse("ftp://x").is_err());
        assert!(ServerUrls::parse("wss://").is_err());
    }

    #[test]
    fn room_codes_are_cleaned_or_refused() {
        assert_eq!(clean_room_code(" k7q-2zp ").unwrap(), "K7Q2ZP");
        for bad in ["../x/y", "K7Q2Z", "K7Q2ZPX", "K7Q2Z?"] {
            assert_eq!(clean_room_code(bad).unwrap_err(), NetplayError::BadRoomCode);
        }
    }
}
