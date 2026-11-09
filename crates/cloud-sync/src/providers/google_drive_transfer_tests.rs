//! Scripted transfer limits: ranged downloads resumed after a 503 / a body cut
//! short, per-range timeouts sized to the bytes, and the pre-play budget view
//! (short timeouts, no retry sleep past the deadline, shared folder cache).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::google_drive_downloads::DOWNLOAD_RANGE_BYTES;
use super::google_drive_test_support::{files, reply, seeded};
use crate::fake_http_transport::{FakeHttpTransport, ScriptedReply};
use crate::http_transport::{RequestBudget, PRE_PLAY_REQUEST_TIMEOUT};
use crate::{CloudError, CloudStorageProvider, Rev};

fn media(status: u16, bytes: &[u8]) -> ScriptedReply {
    ScriptedReply {
        status,
        headers: Vec::new(),
        body: bytes.to_vec(),
    }
}

#[test]
fn big_download_goes_in_ranges_and_resumes_a_cut_body() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let range = DOWNLOAD_RANGE_BYTES as usize;
    let len = 2 * range + 10;
    let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    let lookup = format!(r#"{{"id":"big","headRevisionId":"h1","size":"{len}"}}"#);
    http.push_json(200, &files(&lookup));
    http.push_reply(media(206, &data[..range]));
    http.push_reply(reply(503, &[("retry-after", "0")], ""));
    http.push_reply(media(206, &data[range..range + 1000])); // connection cut
    http.push_reply(media(206, &data[range + 1000..])); // resumed range runs on
    let mut out = Vec::new();
    let rev = drive.read_to("big.bin", &mut out).unwrap();
    assert_eq!(rev, Rev("h1".into()));
    assert!(out == data, "every byte once, in order");

    let gets: Vec<_> = http.requests().into_iter().skip(1).collect();
    let ranges: Vec<_> = gets.iter().map(|r| r.header("range").unwrap()).collect();
    let r = range;
    assert_eq!(
        ranges,
        [
            format!("bytes=0-{}", r - 1),
            format!("bytes={r}-{}", 2 * r - 1),
            format!("bytes={r}-{}", 2 * r - 1),
            format!("bytes={}-{}", r + 1000, len - 1),
        ]
    );
    assert_eq!(
        gets[0].timeout,
        Some(Duration::from_secs(80)),
        "sized to 5 MiB"
    );
}

#[test]
fn download_that_keeps_arriving_empty_gives_up() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(
        200,
        &files(r#"{"id":"f","headRevisionId":"h1","size":"4"}"#),
    );
    for _ in 0..4 {
        http.push_reply(media(206, b""));
    }
    let err = drive.read_to("f.bin", &mut Vec::new()).unwrap_err();
    assert_eq!(err, CloudError::Offline);
    assert_eq!(http.pending(), 0);
    let small = http.requests()[1].timeout;
    assert_eq!(
        small,
        Some(Duration::from_secs(30)),
        "small range: plain timeout"
    );
}

#[test]
fn pre_play_view_uses_short_timeouts_and_the_shared_folder_cache() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let deadline = Instant::now() + Duration::from_secs(15);
    let pre_play = drive.budgeted(RequestBudget::until(deadline)).unwrap();
    http.push_json(200, &files(""));
    assert!(pre_play.list("").unwrap().is_empty());
    let requests = http.requests();
    assert_eq!(requests.len(), 1, "root chain came from the shared cache");
    assert!(requests[0].timeout.unwrap() <= PRE_PLAY_REQUEST_TIMEOUT);
    assert_eq!(pre_play.account_label(), "me@example.com");
}

#[test]
fn pre_play_view_never_sleeps_past_the_deadline() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let deadline = Instant::now() + Duration::from_millis(300);
    let pre_play = drive.budgeted(RequestBudget::until(deadline)).unwrap();
    http.push_reply(reply(503, &[], "")); // default backoff: 1 s > the 300 ms left
    http.push_json(200, &files(""));
    let started = Instant::now();
    assert!(pre_play.list("").unwrap_err().is_transient());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "no backoff sleep"
    );
    assert_eq!(http.requests().len(), 1, "no retry after the budget");
}

#[test]
fn big_upload_into_a_vanished_cached_folder_re_resolves_once() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files("")); // big.bin not in the cached (trashed) v1
    http.push_reply(reply(404, &[], "")); // session start: parent gone
    http.push_json(200, &files(r#"{"id":"R2"}"#)); // root by marker
    http.push_json(200, &files(r#"{"id":"NC2"}"#));
    http.push_json(200, &files(r#"{"id":"V2"}"#));
    http.push_json(200, &files(""));
    let session = [("location", "https://upload.example/s2")];
    http.push_reply(reply(200, &session, ""));
    http.push_json(200, r#"{"id":"big","headRevisionId":"r1"}"#);
    let rev = drive.write_large("big.bin", &mut &b"abc"[..], 3).unwrap();
    assert_eq!(rev, Rev("r1".into()));
    let requests = http.requests();
    assert_eq!(requests.len(), 8);
    assert!(requests[6].body_text().contains(r#""parents":["V2"]"#));
    assert_eq!(requests[7].body, b"abc");
}
