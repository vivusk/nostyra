//! Scripted Drive replies: root folder lookup / create and the folder cache, list
//! paging, reads, the write pre-check and `q` escaping.

use std::sync::Arc;

use http_download_util::HttpMethod;

use super::google_drive_test_support::{files, is_folder_create, provider, q_of, seeded};
use crate::fake_http_transport::FakeHttpTransport;
use crate::{CloudError, CloudStorageProvider, Rev};

#[test]
fn root_folder_created_once_with_marker_then_cached() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = provider(&http);
    for (step, id) in [("root", "R"), ("nostyra-cloud", "NC"), ("v1", "V1")] {
        http.push_json(200, &files(""));
        http.push_json(200, &format!(r#"{{"id":"{id}"}}"#));
        let _ = step;
    }
    http.push_json(200, &files("")); // format.json not there yet
    http.push_json(200, r#"{"id":"F","headRevisionId":"r1"}"#);
    assert_eq!(
        drive.write("format.json", b"{}", None).unwrap(),
        Rev("r1".into())
    );
    let requests = http.requests();
    assert_eq!(requests.len(), 8);
    assert!(q_of(&requests[0]).contains("appProperties has { key='nostyra' and value='root-v1' }"));
    let root = requests[1].body_text();
    assert!(root.contains(r#""appProperties":{"nostyra":"root-v1"}"#) && !root.contains("parents"));
    assert!(root.contains(r#""name":"Nostyra""#));
    assert!(requests[3].body_text().contains(r#""parents":["R"]"#));
    assert_eq!(requests[1].header("authorization"), Some("Bearer old"));

    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"I","headRevisionId":"r2"}"#);
    drive.write("index.json", b"{}", None).unwrap();
    assert_eq!(http.requests().len(), 10, "folders came from the cache");
    assert!(q_of(&http.requests()[8]).contains("'V1' in parents"));
}

#[test]
fn renamed_root_folder_is_found_by_marker() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = provider(&http);
    http.push_json(
        200,
        &files(r#"{"id":"R","name":"My saves","mimeType":"application/vnd.google-apps.folder"}"#),
    );
    http.push_json(200, &files(r#"{"id":"NC"}"#));
    http.push_json(200, &files(""));
    assert_eq!(
        drive.list("").unwrap_err(),
        CloudError::NotFound,
        "v1 missing, nothing created"
    );
    assert!(!http.requests().iter().any(is_folder_create));
}

#[test]
fn list_pages_and_marks_folders() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(
        200,
        r#"{"nextPageToken":"p2","files":[{"id":"1","name":"games","mimeType":"application/vnd.google-apps.folder"},{"id":"2","name":"index.json"}]}"#,
    );
    http.push_json(
        200,
        &files(r#"{"id":"3","name":"format.json"},{"id":"4","name":"index.json"}"#),
    );
    let entries = drive.list("").unwrap();
    let names: Vec<_> = entries
        .iter()
        .map(|e| (e.name.as_str(), e.is_dir))
        .collect();
    assert_eq!(
        names,
        [
            ("format.json", false),
            ("games", true),
            ("index.json", false)
        ]
    );
    let requests = http.requests();
    assert_eq!(q_of(&requests[0]), "'v1' in parents and trashed = false");
    assert!(requests[1].url.contains("pageToken=p2"));
    assert!(requests[0].url.contains("headRevisionId"));
}

#[test]
fn read_uses_the_newest_copy_and_its_head_revision() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(
        200,
        &files(r#"{"id":"new","headRevisionId":"h2"},{"id":"old","headRevisionId":"h1"}"#),
    );
    http.push_json(200, r#"{"generation":3}"#);
    let (bytes, rev) = drive.read("index.json").unwrap();
    assert_eq!(
        (bytes.as_slice(), rev),
        (&br#"{"generation":3}"#[..], Rev("h2".into()))
    );
    let requests = http.requests();
    assert!(q_of(&requests[0]).starts_with("name = 'index.json' and 'v1' in parents"));
    assert!(requests[0].url.contains("orderBy=modifiedTime%20desc"));
    assert!(requests[1].url.ends_with("/files/new?alt=media"));
    http.push_json(200, &files(""));
    assert_eq!(
        drive.read("missing.json").unwrap_err(),
        CloudError::NotFound
    );
}

#[test]
fn oversized_metadata_is_refused_before_or_during_download() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let cap = crate::metadata_read_cap::MAX_METADATA_BYTES;
    let lookup = format!(r#"{{"id":"m","headRevisionId":"h1","size":"{}"}}"#, cap + 1);
    http.push_json(200, &files(&lookup));
    let err = drive.read("manifest.json").unwrap_err();
    assert!(matches!(err, CloudError::Other(msg) if msg.contains("8 MiB")));
    assert_eq!(
        http.requests().len(),
        1,
        "no media GET for a declared big size"
    );

    http.push_json(200, &files(r#"{"id":"n","headRevisionId":"h1"}"#)); // no size
    http.push_json(200, &" ".repeat(cap as usize + 1));
    let err = drive.read("index.json").unwrap_err();
    assert!(matches!(err, CloudError::Other(msg) if msg.contains("8 MiB")));
}

#[test]
fn rev_pre_check_refuses_a_changed_file_before_any_upload() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files(r#"{"id":"M","headRevisionId":"theirs"}"#));
    let stale = Rev("mine".into());
    assert_eq!(
        drive
            .write("manifest.json", b"x", Some(&stale))
            .unwrap_err(),
        CloudError::RevMismatch
    );
    assert_eq!(http.requests().len(), 1, "no PATCH after a mismatch");
    http.push_json(200, &files(""));
    assert_eq!(
        drive
            .write("manifest.json", b"x", Some(&stale))
            .unwrap_err(),
        CloudError::RevMismatch
    );

    http.push_json(200, &files(r#"{"id":"M","headRevisionId":"mine"}"#));
    http.push_json(200, r#"{"id":"M","headRevisionId":"next"}"#);
    assert_eq!(
        drive.write("manifest.json", b"x", Some(&stale)).unwrap(),
        Rev("next".into())
    );
    let patch = http.requests().pop().unwrap();
    assert_eq!(patch.method, HttpMethod::Patch);
    assert!(patch
        .url
        .contains("/upload/drive/v3/files/M?uploadType=multipart"));
    assert!(
        patch.body_text().contains("\r\n{}\r\n"),
        "update metadata is empty"
    );
}

#[test]
fn names_are_escaped_in_queries() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files(""));
    assert_eq!(
        drive.read("it's a 'save'.srm").unwrap_err(),
        CloudError::NotFound
    );
    assert!(q_of(&http.requests()[0]).starts_with(r"name = 'it\'s a \'save\'.srm' and"));
}
