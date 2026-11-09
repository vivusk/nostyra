//! Shared helpers of the scripted Google Drive tests: a provider over a
//! `FakeHttpTransport` (fresh access token, optionally the root folder chain
//! cached), reply builders and request inspectors.

use std::sync::Arc;
use std::time::{Duration, Instant};

use http_download_util::HttpMethod;
use percent_encoding::percent_decode_str;

use super::google_drive_account::google_drive_endpoints;
use super::google_drive_provider::GoogleDriveProvider;
use crate::fake_http_transport::{FakeHttpTransport, RecordedRequest, ScriptedReply};
use crate::{AccessToken, ClientCredentials, OAuthSession, SecretString};

/// A provider over `http` with a fresh access token `old` (no token call needed).
pub(super) fn provider(http: &Arc<FakeHttpTransport>) -> GoogleDriveProvider {
    let endpoints = google_drive_endpoints(ClientCredentials {
        client_id: "cid".into(),
        client_secret: Some(SecretString::new("GOCSPX-test")),
    });
    let access = AccessToken {
        value: SecretString::new("old"),
        expires_at: Instant::now() + Duration::from_secs(3600),
    };
    let session = OAuthSession::new(
        endpoints,
        http.clone(),
        SecretString::new("1//refresh"),
        Some(access),
        Box::new(|_| Ok(())),
    );
    GoogleDriveProvider::new(session, "me@example.com")
}

/// [`provider`] with the root chain already cached (`root` / `nc` / `v1`).
pub(super) fn seeded(http: &Arc<FakeHttpTransport>) -> GoogleDriveProvider {
    let drive = provider(http);
    let mut folders = drive.folders();
    folders.insert(String::new(), "root".into());
    folders.insert("nostyra-cloud".into(), "nc".into());
    folders.insert("nostyra-cloud/v1".into(), "v1".into());
    drop(folders);
    drive
}

pub(super) fn files(json: &str) -> String {
    format!(r#"{{"files":[{json}]}}"#)
}

/// The decoded `q` of a `files.list` request.
pub(super) fn q_of(request: &RecordedRequest) -> String {
    let query = request.url.split_once('?').unwrap().1;
    let q = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("q="))
        .unwrap();
    percent_decode_str(q).decode_utf8().unwrap().into_owned()
}

pub(super) fn is_folder_create(request: &RecordedRequest) -> bool {
    request.method == HttpMethod::Post && request.url.ends_with("/drive/v3/files?fields=id")
}

pub(super) fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> ScriptedReply {
    ScriptedReply {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: body.as_bytes().to_vec(),
    }
}

/// Lookups + creates of `blobs` / `1-x` (seeded root chain), then the file lookup.
pub(super) fn script_blob_dir(http: &FakeHttpTransport) {
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"B"}"#);
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"X"}"#);
    http.push_json(200, &files(""));
}
