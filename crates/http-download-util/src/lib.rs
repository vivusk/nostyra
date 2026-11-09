//! Blocking HTTPS helpers shared by `core-manager` (core downloads),
//! `artwork-cache` (thumbnails) and `cloud-sync` (REST APIs): one configured `ureq`
//! client with an offline switch, a generic request, a retry loop honouring
//! `Retry-After`, a size-capped cancellable copy, and a download error type that
//! separates "not there" from "try again later".
//!
//! No async runtime: callers run downloads on worker threads.

#![deny(unsafe_code)]

mod capped_copy;
mod download_error_classification;
mod http_client;
mod http_generic_request;
mod retry_after_header;
mod retry_with_backoff;

pub use capped_copy::{copy_with_limit, read_body_with_limit};
pub use download_error_classification::DownloadError;
pub use http_client::{build_http_client, HttpClient, HttpResponse};
pub use http_generic_request::{HttpMethod, HttpReply, HttpRequest, RequestBody};
pub use retry_after_header::parse_retry_after;
pub use retry_with_backoff::{
    backoff_delay, retry_with_backoff, retry_with_clock, RetryClass, RetryHint, RetryPolicy,
    DEFAULT_MAX_TRIES, MAX_BACKOFF, MAX_RETRY_WAIT,
};
