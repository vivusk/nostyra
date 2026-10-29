//! Blocking HTTPS helpers shared by `core-manager` (core downloads) and
//! `artwork-cache` (thumbnails): one configured `ureq` client with an offline
//! switch, a size-capped cancellable copy, and a download error type that
//! separates "not there" from "try again later".
//!
//! No async runtime: callers run downloads on worker threads.

#![deny(unsafe_code)]

mod capped_copy;
mod download_error_classification;
mod http_client;

pub use capped_copy::{copy_with_limit, read_body_with_limit};
pub use download_error_classification::DownloadError;
pub use http_client::{build_http_client, HttpClient, HttpResponse};
