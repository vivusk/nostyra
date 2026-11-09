//! Generic HTTPS request on the shared client (cloud-save REST APIs): any method,
//! extra headers, an empty / in-memory / streamed body and per-call timeouts. The
//! reply comes back for **every** status (REST APIs put their error in 4xx bodies:
//! the caller classifies, e.g. with [`DownloadError::from_status`]); only transport
//! failures are errors. Same guards as `get`: offline switch, HTTPS only. Only GET
//! follows redirects (download URLs); other methods get the 3xx itself (Drive's
//! resumable upload answers `308 Resume Incomplete` without a `Location`).

use std::io::Read;
use std::time::Duration;

use ureq::http::{Method, Request};

use crate::{DownloadError, HttpClient};

/// HTTP method of a [`HttpRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    fn as_method(self) -> Method {
        match self {
            Self::Get => Method::GET,
            Self::Post => Method::POST,
            Self::Put => Method::PUT,
            Self::Patch => Method::PATCH,
            Self::Delete => Method::DELETE,
        }
    }

    /// Methods that always carry a body (`Content-Length: 0` when empty).
    fn sends_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }
}

/// Request body.
pub enum RequestBody<'a> {
    Empty,
    Bytes(&'a [u8]),
    /// Streamed upload of exactly `len` bytes (sent as `Content-Length`).
    Reader {
        reader: &'a mut dyn Read,
        len: u64,
    },
}

/// One request. `connect_timeout` / `timeout` (whole call, body included) override
/// the client defaults when set.
pub struct HttpRequest<'a> {
    pub method: HttpMethod,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: RequestBody<'a>,
    pub connect_timeout: Option<Duration>,
    pub timeout: Option<Duration>,
}

/// Status, response headers (lower-case names) and the unread body.
pub struct HttpReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

impl HttpReply {
    /// First header named `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl std::fmt::Debug for HttpReply {
    /// Headers are left out: they may carry tokens or signed URLs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpReply")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    /// Sends `request` and returns the reply for every HTTP status.
    pub fn request(&self, request: HttpRequest<'_>) -> Result<HttpReply, DownloadError> {
        self.refuse_before_io(request.url)?;
        let mut builder = Request::builder()
            .method(request.method.as_method())
            .uri(request.url);
        for (name, value) in request.headers {
            builder = builder.header(*name, *value);
        }
        let options = RunOptions {
            connect: request.connect_timeout,
            total: request.timeout,
            follow_redirects: request.method == HttpMethod::Get,
        };
        let response = match request.body {
            RequestBody::Empty if request.method.sends_body() => {
                self.run(with_body(builder, &[][..])?, options, None)
            }
            RequestBody::Empty => self.run(with_body(builder, ())?, options, None),
            RequestBody::Bytes(bytes) => self.run(with_body(builder, bytes)?, options, None),
            RequestBody::Reader { reader, len } => {
                let body = ureq::SendBody::from_reader(reader);
                self.run(with_body(builder, body)?, options, Some(len))
            }
        }?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
            })
            .collect();
        Ok(HttpReply {
            status,
            headers,
            body: Box::new(response.into_body().into_reader()),
        })
    }

    /// Applies per-call timeouts, the redirect rule and (streamed bodies) an explicit
    /// `Content-Length`, then runs. In-memory bodies get their length from ureq.
    fn run<S: ureq::AsSendBody>(
        &self,
        mut request: Request<S>,
        RunOptions {
            connect,
            total,
            follow_redirects,
        }: RunOptions,
        content_length: Option<u64>,
    ) -> Result<ureq::http::Response<ureq::Body>, DownloadError> {
        if let Some(len) = content_length {
            request
                .headers_mut()
                .insert(ureq::http::header::CONTENT_LENGTH, len.into());
        }
        let mut config = self.agent.configure_request(request);
        if let Some(connect) = connect {
            config = config.timeout_connect(Some(connect));
        }
        if let Some(total) = total {
            config = config.timeout_global(Some(total));
        }
        if !follow_redirects {
            config = config.max_redirects(0);
        }
        Ok(self.agent.run(config.build())?)
    }
}

/// Per-call settings of [`HttpClient::run`].
#[derive(Clone, Copy)]
struct RunOptions {
    connect: Option<Duration>,
    total: Option<Duration>,
    follow_redirects: bool,
}

fn with_body<S>(
    builder: ureq::http::request::Builder,
    body: S,
) -> Result<Request<S>, DownloadError> {
    builder
        .body(body)
        .map_err(|err| DownloadError::Permanent(format!("bad request: {err}")))
}
