//! How rcheevos requests reach a server: the `Send + Sync` [`ServerTransport`] trait
//! (runs on the HTTP worker or inline), the real HTTPS transport over the shared
//! `http-download-util` client, and the retry classification rc_client itself uses.

use http_download_util::{build_http_client, read_body_with_limit, DownloadError, HttpClient};

/// Status for "no HTTP response" that rc_client retries (`RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR`).
pub const STATUS_NO_RESPONSE: i32 = -2;
/// Body sent with [`STATUS_NO_RESPONSE`] when the request timed out / failed in transit.
/// rc_client shows the body as the error text, so it is short and never empty (D5).
pub const BODY_TIMEOUT: &str = "timeout";
/// Body sent with [`STATUS_NO_RESPONSE`] when the network is switched off or unreachable.
pub const BODY_OFFLINE: &str = "offline";
/// Body sent with [`STATUS_NO_RESPONSE`] for requests still pending at client shutdown.
pub const BODY_SHUTDOWN: &str = "shutdown";
/// Largest response accepted (a big game set is a few hundred KiB).
pub(crate) const RESPONSE_LIMIT_BYTES: u64 = 16 * 1024 * 1024;

/// One rcheevos request (copied out of `rc_api_request_t`; owned, `Send`).
#[derive(Clone, PartialEq, Eq)]
pub struct ServerRequest {
    pub url: String,
    /// Form body (`r=<kind>&u=…`); empty for a body-less request.
    pub post_data: String,
    pub content_type: String,
}

impl std::fmt::Debug for ServerRequest {
    /// Only the request kind: the body carries the token (or the login password).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerRequest")
            .field("url", &self.url)
            .field("kind", &request_kind(&self.post_data))
            .finish_non_exhaustive()
    }
}

/// Answer for one request: an HTTP status with its body, or [`STATUS_NO_RESPONSE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerResponse {
    pub status: i32,
    pub body: String,
}

impl ServerResponse {
    /// Transport failure response (status −2 with a short reason body).
    pub fn no_response(reason: &str) -> Self {
        Self {
            status: STATUS_NO_RESPONSE,
            body: reason.to_owned(),
        }
    }
}

/// Executes requests. Implementations block; `Send + Sync` so one instance can serve
/// the HTTP worker thread. Never touches rc_client or callback data.
pub trait ServerTransport: Send + Sync {
    fn call(&self, request: &ServerRequest) -> ServerResponse;

    /// GET `url` (badge images on the media server, never through rc_client): the
    /// body of a 2xx answer up to `limit` bytes, else a short reason.
    fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let _ = (url, limit);
        Err("downloads not supported by this transport".into())
    }
}

/// Value of a form field (`key=value` pairs joined by `&`; values stay URL-encoded).
pub fn form_field<'a>(post_data: &'a str, key: &str) -> Option<&'a str> {
    post_data
        .split('&')
        .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
}

/// The request kind (`r=` field), e.g. `login2`, `awardachievement`.
pub fn request_kind(post_data: &str) -> &str {
    form_field(post_data, "r").unwrap_or("")
}

/// rc_client's own retry rule (`rc_client_should_retry`, v12.5.0): gateway / overload
/// statuses, an explicit −2, or no body at all. −1 (client error) is final.
pub fn is_retryable_response(response: &ServerResponse) -> bool {
    match response.status {
        429 | 502 | 503 | 504 | 521..=525 | STATUS_NO_RESPONSE => true,
        -1 => false,
        _ => response.body.is_empty(),
    }
}

/// Real HTTPS transport (ureq through `http-download-util`).
#[derive(Debug, Clone)]
pub struct UreqTransport {
    http: HttpClient,
}

impl UreqTransport {
    /// Builds the client with `user_agent`; honours `NOSTYRA_OFFLINE=1` (every request
    /// then answers −2 `offline` without opening a socket).
    pub fn new(user_agent: &str) -> Self {
        let offline = std::env::var("NOSTYRA_OFFLINE").is_ok_and(|value| value == "1");
        Self::with_client(build_http_client(user_agent, offline))
    }

    /// Wraps an already configured client.
    pub fn with_client(http: HttpClient) -> Self {
        Self { http }
    }
}

impl ServerTransport for UreqTransport {
    fn call(&self, request: &ServerRequest) -> ServerResponse {
        match self
            .http
            .post_form(&request.url, &request.post_data, RESPONSE_LIMIT_BYTES)
        {
            Ok((status, body)) => ServerResponse {
                status: i32::from(status),
                body: String::from_utf8_lossy(&body).into_owned(),
            },
            Err(DownloadError::Transient(reason)) if reason == BODY_OFFLINE => {
                ServerResponse::no_response(BODY_OFFLINE)
            }
            Err(err) => {
                tracing::debug!(kind = request_kind(&request.post_data), %err, "achievement request failed");
                ServerResponse::no_response(BODY_TIMEOUT)
            }
        }
    }

    fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let never_cancelled = std::sync::atomic::AtomicBool::new(false);
        let response = self.http.get(url, &[]).map_err(|err| err.to_string())?;
        read_body_with_limit(response, limit, &never_cancelled).map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_fields_and_kind_are_parsed() {
        let post = "r=awardachievement&u=Spike&a=101&h=0";
        assert_eq!(request_kind(post), "awardachievement");
        assert_eq!(form_field(post, "a"), Some("101"));
        assert_eq!(form_field(post, "x"), None);
        assert_eq!(request_kind("u=x"), "");
    }

    #[test]
    fn retry_rule_matches_rc_client() {
        let resp = |status, body: &str| ServerResponse {
            status,
            body: body.into(),
        };
        assert!(is_retryable_response(&resp(-2, "timeout")));
        assert!(is_retryable_response(&resp(503, "{}")));
        assert!(is_retryable_response(&resp(200, "")));
        assert!(!is_retryable_response(&resp(-1, "")));
        assert!(!is_retryable_response(&resp(200, "{\"Success\":false}")));
    }

    #[test]
    fn offline_transport_answers_minus_two_offline() {
        let transport = UreqTransport::with_client(build_http_client("Nostyra-test", true));
        let response = transport.call(&ServerRequest {
            url: "https://retroachievements.org/dorequest.php".into(),
            post_data: "r=login2&u=x&t=y".into(),
            content_type: "application/x-www-form-urlencoded".into(),
        });
        assert_eq!(response, ServerResponse::no_response(BODY_OFFLINE));
        let badge = transport.download("https://media.retroachievements.org/Badge/1.png", 1024);
        assert!(badge.is_err(), "offline downloads fail before any socket");
    }

    #[test]
    fn debug_output_hides_the_body() {
        let request = ServerRequest {
            url: "https://x/dorequest.php".into(),
            post_data: "r=login2&u=me&p=secret".into(),
            content_type: String::new(),
        };
        let text = format!("{request:?}");
        assert!(text.contains("login2") && !text.contains("secret"));
    }
}
