//! ICE server list with time-limited TURN credentials (coturn REST API / `use-auth-secret`):
//! `username = "<exp>:<peer_id>"`, `credential = base64(HMAC-SHA1(TURN_SECRET, username))`.
//! The credential is a secret: it travels to the runner in an env var, never in logs.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;

/// TURN credential lifetime.
pub const TURN_TTL_SECS: u64 = 2 * 60 * 60;

/// coturn listening port (plain STUN / TURN, udp + tcp; no TURNS in v1).
const TURN_PORT: u16 = 3478;

/// One `RTCIceServer` entry, JSON shape the launcher forwards verbatim to the runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// coturn's REST-API password for `username`.
pub fn turn_credential(turn_secret: &str, username: &str) -> String {
    // HMAC accepts keys of any length, so `new_from_slice` cannot fail here.
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(turn_secret.as_bytes())
        .unwrap_or_else(|_| unreachable!("HMAC takes any key length"));
    mac.update(username.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

/// STUN entry + TURN entry (udp + tcp) on `turn_host`, credentials valid for 2 h.
pub fn ice_servers(
    turn_host: &str,
    turn_secret: &str,
    peer_id: &str,
    now_unix: u64,
) -> Vec<IceServer> {
    let username = format!("{}:{peer_id}", now_unix + TURN_TTL_SECS);
    let credential = turn_credential(turn_secret, &username);
    vec![
        IceServer {
            urls: vec![format!("stun:{turn_host}:{TURN_PORT}")],
            username: None,
            credential: None,
        },
        IceServer {
            urls: vec![
                format!("turn:{turn_host}:{TURN_PORT}?transport=udp"),
                format!("turn:{turn_host}:{TURN_PORT}?transport=tcp"),
            ],
            username: Some(username),
            credential: Some(credential),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_matches_coturn_algorithm_fixed_vector() {
        // printf '%s' "1800007200:abcd" | openssl dgst -sha1 -hmac "turn-secret-for-tests" -binary | base64
        assert_eq!(
            turn_credential("turn-secret-for-tests", "1800007200:abcd"),
            "N35PMr6DT9ChFmEUgFHYiEJ69xM="
        );
    }

    #[test]
    fn ice_list_has_stun_and_turn_udp_tcp_with_two_hour_username() {
        let list = ice_servers(
            "netplay.nostyra.com",
            "turn-secret-for-tests",
            "abcd",
            1_800_000_000,
        );
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].urls, ["stun:netplay.nostyra.com:3478"]);
        assert_eq!(list[0].username, None);
        assert_eq!(
            list[1].urls,
            [
                "turn:netplay.nostyra.com:3478?transport=udp",
                "turn:netplay.nostyra.com:3478?transport=tcp"
            ]
        );
        assert_eq!(list[1].username.as_deref(), Some("1800007200:abcd"));
        assert_eq!(
            list[1].credential.as_deref(),
            Some("N35PMr6DT9ChFmEUgFHYiEJ69xM=")
        );
        let json = serde_json::to_value(&list[0]).unwrap();
        assert!(
            json.get("username").is_none(),
            "STUN entry carries no credentials"
        );
    }
}
