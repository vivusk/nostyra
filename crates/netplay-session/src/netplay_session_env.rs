//! `NOSTYRA_NETPLAY_SESSION`: everything the runner needs to open the signaling
//! WebSocket and ICE, handed over by the launcher as one env JSON (never as args:
//! the token and TURN credential are secrets). The runner makes no REST call.

use serde::{Deserialize, Serialize};

use crate::room_api_types::{IceServer, SeatRole};
use crate::NetplayError;

/// Env var holding the JSON.
pub const ENV_KEY: &str = "NOSTYRA_NETPLAY_SESSION";

/// Room seat handed from the launcher to the runner.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetplaySessionEnv {
    /// WebSocket base, e.g. `wss://netplay.nostyra.com/v1/ws` (room id appended).
    pub server_ws_url: String,
    pub room_id: String,
    /// Room code (host only: shown in the waiting room).
    #[serde(default)]
    pub code: Option<String>,
    pub token: String,
    pub role: SeatRole,
    /// Room player cap, known before `retro_load_game` for every role (F21).
    pub max_players: u8,
    pub ice_servers: Vec<IceServer>,
    /// Token expiry, Unix seconds.
    pub expires_at: u64,
}

impl NetplaySessionEnv {
    /// Reads and validates [`ENV_KEY`].
    pub fn from_env() -> Result<Self, NetplayError> {
        let json = std::env::var(ENV_KEY)
            .map_err(|_| NetplayError::SessionEnv("NOSTYRA_NETPLAY_SESSION is not set"))?;
        Self::from_json(&json)
    }

    /// Parses and validates the JSON (errors never echo the input).
    pub fn from_json(json: &str) -> Result<Self, NetplayError> {
        let env: Self = serde_json::from_str(json)
            .map_err(|_| NetplayError::SessionEnv("NOSTYRA_NETPLAY_SESSION is not valid JSON"))?;
        env.validate()?;
        Ok(env)
    }

    /// The JSON the launcher puts in the env.
    pub fn to_json(&self) -> Result<String, NetplayError> {
        serde_json::to_string(self).map_err(|_| NetplayError::SessionEnv("cannot encode"))
    }

    /// Range / charset checks: the ids end up in a URL.
    pub fn validate(&self) -> Result<(), NetplayError> {
        let bad = NetplayError::SessionEnv;
        if !(2..=4).contains(&self.max_players) {
            return Err(bad("max_players must be 2..=4"));
        }
        let url = self.server_ws_url.as_str();
        if !(url.starts_with("wss://") || url.starts_with("ws://")) || url.contains(['?', '#']) {
            return Err(bad("server_ws_url must be a ws:// or wss:// base url"));
        }
        if !url_safe(&self.room_id) || !url_safe(&self.token) {
            return Err(bad("room_id / token have unexpected characters"));
        }
        if self.ice_servers.iter().any(|ice| ice.urls.is_empty()) {
            return Err(bad("ice server without urls"));
        }
        Ok(())
    }

    /// `server_ws_url/room_id?token=…` — the signaling url (a secret: never logged).
    pub fn ws_url(&self) -> String {
        let base = self.server_ws_url.trim_end_matches('/');
        format!("{base}/{}?token={}", self.room_id, self.token)
    }
}

/// Non-empty, `[A-Za-z0-9._-]` only (hex room ids, base64url tokens).
fn url_safe(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

impl std::fmt::Debug for NetplaySessionEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetplaySessionEnv")
            .field("server_ws_url", &self.server_ws_url)
            .field("room_id", &"<redacted>")
            .field("code", &self.code.as_ref().map(|_| "<redacted>"))
            .field("token", &"<redacted>")
            .field("role", &self.role)
            .field("max_players", &self.max_players)
            .field("ice_servers", &self.ice_servers)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn sample() -> NetplaySessionEnv {
        NetplaySessionEnv {
            server_ws_url: "wss://netplay.example/v1/ws".into(),
            room_id: "a1b2c3".into(),
            code: Some("K7Q2ZP".into()),
            token: "cGF5bG9hZA.c2ln".into(),
            role: SeatRole::Host,
            max_players: 4,
            ice_servers: vec![IceServer {
                urls: vec!["turn:turn.example:3478?transport=udp".into()],
                username: Some("1700000000:peer".into()),
                credential: Some("TURNSECRET".into()),
            }],
            expires_at: 1_700_000_000,
        }
    }

    #[test]
    fn json_round_trip_keeps_max_players() {
        let env = sample();
        let back = NetplaySessionEnv::from_json(&env.to_json().unwrap()).unwrap();
        assert_eq!(back, env);
        assert_eq!(back.max_players, 4);
        let url = back.ws_url();
        assert_eq!(
            url,
            "wss://netplay.example/v1/ws/a1b2c3?token=cGF5bG9hZA.c2ln"
        );
    }

    #[test]
    fn max_players_outside_range_is_refused() {
        for players in [0, 1, 5] {
            let env = NetplaySessionEnv {
                max_players: players,
                ..sample()
            };
            let err = NetplaySessionEnv::from_json(&env.to_json().unwrap()).unwrap_err();
            assert!(matches!(err, NetplayError::SessionEnv(_)), "{players}");
        }
    }

    #[test]
    fn missing_or_malformed_env_is_a_session_env_error() {
        for json in ["", "{", r#"{"room_id":"x"}"#, "[1,2]"] {
            let err = NetplaySessionEnv::from_json(json).unwrap_err();
            assert!(matches!(err, NetplayError::SessionEnv(_)), "{json}");
        }
        let bad_token = NetplaySessionEnv {
            token: "a b&c".into(),
            ..sample()
        };
        assert!(bad_token.validate().is_err());
        let bad_url = NetplaySessionEnv {
            server_ws_url: "https://x/v1/ws".into(),
            ..sample()
        };
        assert!(bad_url.validate().is_err());
    }

    #[test]
    fn debug_hides_token_code_and_credentials() {
        let text = format!("{:?}", sample());
        for secret in [
            "cGF5bG9hZA",
            "K7Q2ZP",
            "TURNSECRET",
            "1700000000:peer",
            "a1b2c3",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        assert!(text.contains("max_players: 4"));
    }
}
