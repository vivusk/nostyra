//! The one HTTPS client artwork (and future network features) go through:
//! fixed timeouts so a dead network fails within one request timeout, an
//! identifiable User-Agent, and an `offline` switch that fails before any
//! socket is opened (`NOSTYRA_OFFLINE=1` in the launcher, offline tests).

use std::time::Duration;

use crate::DownloadError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Time to the response headers once connected.
const HEADERS_TIMEOUT: Duration = Duration::from_secs(15);
const BODY_TIMEOUT: Duration = Duration::from_secs(60);
/// Whole-request bound (redirects included).
const GLOBAL_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_REDIRECTS: u32 = 5;
/// Keep-alive slots per host; matches the artwork worker count.
const IDLE_CONNECTIONS_PER_HOST: usize = 4;

/// A response with a 2xx or 304 status; the body is still unread.
pub type HttpResponse = ureq::http::Response<ureq::Body>;

/// Shared blocking HTTPS client. Cheap to clone (the agent pools connections
/// and is `Send + Sync`), so one instance serves every worker thread.
#[derive(Debug, Clone)]
pub struct HttpClient {
    agent: ureq::Agent,
    offline: bool,
}

/// Builds the client. `offline = true` makes every request fail with
/// `DownloadError::Transient("offline")` without touching the network; the
/// caller decides (the launcher reads `NOSTYRA_OFFLINE=1`).
pub fn build_http_client(user_agent: &str, offline: bool) -> HttpClient {
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .user_agent(user_agent)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(HEADERS_TIMEOUT))
        .timeout_recv_body(Some(BODY_TIMEOUT))
        .timeout_global(Some(GLOBAL_TIMEOUT))
        .max_redirects(MAX_REDIRECTS)
        .max_idle_connections_per_host(IDLE_CONNECTIONS_PER_HOST)
        // Statuses are classified here so `Retry-After` stays readable.
        .http_status_as_error(false)
        .build();
    HttpClient {
        agent: ureq::Agent::new_with_config(config),
        offline,
    }
}

impl HttpClient {
    /// True when built with the offline switch (requests never leave the process).
    pub fn is_offline(&self) -> bool {
        self.offline
    }

    /// GET `url` with extra request `headers`. Returns the response for 2xx
    /// and 304 (conditional GET); every other outcome becomes a classified
    /// [`DownloadError`]. Non-HTTPS URLs are refused before any I/O.
    pub fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, DownloadError> {
        self.refuse_before_io(url)?;
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.call()?;
        let status = response.status();
        if status.is_success() || status == ureq::http::StatusCode::NOT_MODIFIED {
            return Ok(response);
        }
        let retry_after = response
            .headers()
            .get(ureq::http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        tracing::debug!(url, status = status.as_u16(), "http request failed");
        Err(DownloadError::from_status(status.as_u16(), retry_after))
    }

    /// POSTs an `application/x-www-form-urlencoded` `form` body (RetroAchievements
    /// `dorequest.php`) and returns `(status, body)` for **every** HTTP status: that
    /// API puts its JSON error (e.g. invalid credentials) in 4xx bodies, so the caller
    /// classifies. Only transport failures (offline switch, non-HTTPS, DNS, connect,
    /// timeouts, body over `limit` bytes) are errors.
    pub fn post_form(
        &self,
        url: &str,
        form: &str,
        limit: u64,
    ) -> Result<(u16, Vec<u8>), DownloadError> {
        self.refuse_before_io(url)?;
        let response = self
            .agent
            .post(url)
            .content_type("application/x-www-form-urlencoded")
            .send(form)?;
        let status = response.status().as_u16();
        let never_cancelled = std::sync::atomic::AtomicBool::new(false);
        let body = crate::read_body_with_limit(response, limit, &never_cancelled)?;
        Ok((status, body))
    }

    /// Guards every request before a socket is opened: offline switch, HTTPS only.
    fn refuse_before_io(&self, url: &str) -> Result<(), DownloadError> {
        if self.offline {
            return Err(DownloadError::Transient("offline".into()));
        }
        if !url.starts_with("https://") {
            return Err(DownloadError::Permanent(format!(
                "refusing non-HTTPS URL {url}"
            )));
        }
        Ok(())
    }
}
