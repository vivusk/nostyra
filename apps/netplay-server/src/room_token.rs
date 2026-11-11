//! Short-lived room tokens: `base64url(payload) "." base64url(HMAC-SHA256(secret, payload))`
//! with payload `v1|room_id|peer_id|role|exp`. Verified once at the WebSocket upgrade
//! (and for `/start`); never refreshed.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::seat_role::SeatRole;

/// Token lifetime: covers lobby time (cloud pull, dialogs, runner launch).
pub const TOKEN_TTL_SECS: u64 = 30 * 60;
/// [`TOKEN_TTL_SECS`] as a `Duration`.
pub const TOKEN_TTL: std::time::Duration = std::time::Duration::from_secs(TOKEN_TTL_SECS);

const VERSION: &str = "v1";

/// What a valid token says about its holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenClaims {
    pub room_id: String,
    pub peer_id: String,
    pub role: SeatRole,
    /// Unix seconds after which the token is refused.
    pub exp: u64,
}

/// Why a token was refused (all map to 403; the details never reach logs or clients).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    Malformed,
    BadSignature,
    Expired,
}

/// Signs a token for `peer_id` in `room_id`, valid until `now_unix + TOKEN_TTL_SECS`.
pub fn sign_token(
    secret: &[u8],
    room_id: &str,
    peer_id: &str,
    role: SeatRole,
    now_unix: u64,
) -> String {
    let exp = now_unix + TOKEN_TTL_SECS;
    let payload = format!("{VERSION}|{room_id}|{peer_id}|{}|{exp}", role.as_str());
    let mac = mac_over(secret, payload.as_bytes()).finalize().into_bytes();
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(mac)
    )
}

/// Checks signature (constant time) and expiry, then returns the claims.
pub fn verify_token(secret: &[u8], token: &str, now_unix: u64) -> Result<TokenClaims, TokenError> {
    let (payload_b64, mac_b64) = token.split_once('.').ok_or(TokenError::Malformed)?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| TokenError::Malformed)?;
    let mac = URL_SAFE_NO_PAD
        .decode(mac_b64)
        .map_err(|_| TokenError::Malformed)?;
    mac_over(secret, &payload)
        .verify_slice(&mac)
        .map_err(|_| TokenError::BadSignature)?;
    let claims = parse_payload(&payload).ok_or(TokenError::Malformed)?;
    if now_unix >= claims.exp {
        return Err(TokenError::Expired);
    }
    Ok(claims)
}

fn mac_over(secret: &[u8], payload: &[u8]) -> Hmac<Sha256> {
    // HMAC accepts keys of any length, so `new_from_slice` cannot fail here.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret)
        .unwrap_or_else(|_| unreachable!("HMAC takes any key length"));
    mac.update(payload);
    mac
}

fn parse_payload(payload: &[u8]) -> Option<TokenClaims> {
    let text = std::str::from_utf8(payload).ok()?;
    let mut parts = text.split('|');
    let (version, room_id, peer_id, role, exp) = (
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
    );
    if version != VERSION || parts.next().is_some() {
        return None;
    }
    Some(TokenClaims {
        room_id: room_id.to_owned(),
        peer_id: peer_id.to_owned(),
        role: SeatRole::parse(role)?,
        exp: exp.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
    const NOW: u64 = 1_800_000_000;

    #[test]
    fn round_trip_keeps_claims() {
        let token = sign_token(SECRET, "room1", "peer1", SeatRole::Host, NOW);
        let claims = verify_token(SECRET, &token, NOW + 10).unwrap();
        assert_eq!(claims.room_id, "room1");
        assert_eq!(claims.peer_id, "peer1");
        assert_eq!(claims.role, SeatRole::Host);
        assert_eq!(claims.exp, NOW + TOKEN_TTL_SECS);
    }

    #[test]
    fn expires_after_thirty_minutes() {
        let token = sign_token(SECRET, "r", "p", SeatRole::Player, NOW);
        assert!(verify_token(SECRET, &token, NOW + 29 * 60 + 59).is_ok());
        assert_eq!(
            verify_token(SECRET, &token, NOW + 30 * 60),
            Err(TokenError::Expired)
        );
    }

    #[test]
    fn wrong_secret_or_tampering_is_refused() {
        let token = sign_token(SECRET, "r", "p", SeatRole::Spectator, NOW);
        let other = b"another-secret-another-secret-xx";
        assert_eq!(
            verify_token(other, &token, NOW),
            Err(TokenError::BadSignature)
        );
        let (_, mac) = token.split_once('.').unwrap();
        let forged = format!("{}.{mac}", URL_SAFE_NO_PAD.encode("v1|r|p|host|9999999999"));
        assert_eq!(
            verify_token(SECRET, &forged, NOW),
            Err(TokenError::BadSignature)
        );
    }

    #[test]
    fn garbage_is_malformed() {
        assert_eq!(verify_token(SECRET, "", NOW), Err(TokenError::Malformed));
        assert_eq!(verify_token(SECRET, "abc", NOW), Err(TokenError::Malformed));
        assert_eq!(
            verify_token(SECRET, "!!.??", NOW),
            Err(TokenError::Malformed)
        );
    }
}
