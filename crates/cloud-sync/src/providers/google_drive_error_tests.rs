//! Scripted Drive replies: deletes (files, folders + cache), 401 → refresh → retry,
//! quota / rate limits, an expired upload session and a failing upload source.

use std::sync::Arc;
use std::time::Duration;

use http_download_util::HttpMethod;

use super::google_drive_test_support::{files, reply, script_blob_dir, seeded};
use crate::fake_http_transport::FakeHttpTransport;
use crate::http_transport::UPLOAD_CHUNK_BYTES;
use crate::{CloudError, CloudStorageProvider};

#[test]
fn delete_removes_every_copy_and_tolerates_gone_files() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files(r#"{"id":"a"},{"id":"b"}"#));
    http.push_json(204, "");
    http.push_json(
        404,
        r#"{"error":{"code":404,"errors":[{"reason":"notFound"}]}}"#,
    );
    drive.delete("index.json").unwrap();
    let deletes: Vec<_> = http
        .requests()
        .into_iter()
        .filter(|r| r.method == HttpMethod::Delete)
        .collect();
    assert_eq!(deletes.len(), 2);
    assert!(deletes[1].url.ends_with("/drive/v3/files/b"));
}

#[test]
fn delete_dir_is_one_call_and_drops_cached_sub_paths() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let base = "nostyra-cloud/v1/games/s/h/blobs";
    for (key, id) in [
        (base.to_string(), "B"),
        (format!("{base}/1-x"), "G1"),
        (format!("{base}/1-x/PSP"), "P"),
    ] {
        drive.folders().insert(key, id.into());
    }
    http.push_json(204, "");
    drive.delete_dir("games/s/h/blobs/1-x").unwrap();
    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].url.ends_with("/files/G1") && requests[0].method == HttpMethod::Delete);
    let folders = drive.folders().clone();
    assert!(folders.contains_key(base) && !folders.keys().any(|k| k.contains("1-x")));

    http.push_json(200, &files(""));
    drive.delete_dir("games/s/h/blobs/1-x").unwrap();
    assert_eq!(http.requests().len(), 2, "already gone: lookup only");
    assert!(drive.delete_dir("").is_err());
}

#[test]
fn unauthorized_refreshes_once_with_the_secret_then_retries() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(
        401,
        r#"{"error":{"code":401,"errors":[{"reason":"authError"}]}}"#,
    );
    http.push_json(200, r#"{"access_token":"fresh","expires_in":3599}"#);
    http.push_json(200, &files(""));
    assert!(drive.list("").unwrap().is_empty());
    let requests = http.requests();
    assert_eq!(requests[0].header("authorization"), Some("Bearer old"));
    assert_eq!(requests[1].url, "https://oauth2.googleapis.com/token");
    assert!(requests[1]
        .body_text()
        .contains("client_secret=GOCSPX-test"));
    assert_eq!(requests[2].header("authorization"), Some("Bearer fresh"));

    http.push_json(401, "");
    http.push_json(200, r#"{"access_token":"fresh2","expires_in":3599}"#);
    http.push_json(401, "");
    assert_eq!(drive.list("").unwrap_err(), CloudError::AuthExpired);
    http.push_json(401, "");
    http.push_json(
        400,
        r#"{"error":"invalid_grant","error_description":"Bad Request"}"#,
    );
    assert_eq!(drive.list("").unwrap_err(), CloudError::AuthExpired);
}

#[test]
fn refused_recursive_delete_falls_back_to_children_first() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    drive
        .folders()
        .insert("nostyra-cloud/v1/b".into(), "B".into());
    let refused = r#"{"error":{"code":403,"errors":[{"reason":"appNotAuthorizedToChild"}]}}"#;
    http.push_json(403, refused);
    http.push_json(
        200,
        &files(r#"{"id":"S","mimeType":"application/vnd.google-apps.folder"},{"id":"f1"}"#),
    );
    http.push_json(204, ""); // sub-folder S goes whole
    http.push_json(204, ""); // f1
    http.push_json(204, ""); // B, now empty
    drive.delete_dir("b").unwrap();
    let deletes: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|r| r.method == HttpMethod::Delete)
        .map(|r| r.url.rsplit('/').next().unwrap().to_string())
        .collect();
    assert_eq!(deletes, ["B", "S", "f1", "B"]);
    assert_eq!(http.pending(), 0);
}

#[test]
fn quota_and_rate_limits_map_to_their_errors() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files(""));
    http.push_json(
        403,
        r#"{"error":{"code":403,"errors":[{"reason":"storageQuotaExceeded"}]}}"#,
    );
    assert_eq!(
        drive.write("save.srm", b"s", None).unwrap_err(),
        CloudError::QuotaFull
    );
    http.push_reply(crate::fake_http_transport::ScriptedReply {
        status: 429,
        headers: vec![("retry-after".into(), "120".into())],
        body: Vec::new(),
    });
    assert_eq!(
        drive.list("").unwrap_err(),
        CloudError::RateLimited {
            retry_after: Some(Duration::from_secs(120))
        },
        "a wait over a minute is not slept: the next sync retries"
    );
}

#[test]
fn expired_session_is_offline_for_a_later_restart() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let len = UPLOAD_CHUNK_BYTES + 1;
    let data = vec![7u8; len as usize];
    script_blob_dir(&http);
    http.push_reply(reply(200, &[("location", "https://upload.example/s")], ""));
    http.push_reply(reply(404, &[], ""));
    let err = drive
        .write_large("blobs/1-x/big.bin", &mut data.as_slice(), len)
        .unwrap_err();
    assert_eq!(err, CloudError::Offline);
    assert_eq!(http.pending(), 0);
}

#[test]
fn reader_failure_aborts_the_upload() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    script_blob_dir(&http);
    http.push_reply(reply(200, &[("location", "https://upload.example/s")], ""));
    let short = vec![1u8; 10];
    let err = drive
        .write_large("blobs/1-x/big.bin", &mut short.as_slice(), 100)
        .unwrap_err();
    assert!(matches!(err, CloudError::Other(_)), "{err:?}");
}
