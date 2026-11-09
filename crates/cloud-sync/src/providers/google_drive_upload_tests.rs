//! Scripted uploads: multipart body shape, nested folder creation counted once per
//! new folder, resumable chunks resumed from `Range` after a 503.

use std::sync::Arc;

use http_download_util::HttpMethod;

use super::google_drive_test_support::{
    files, is_folder_create, q_of, reply, script_blob_dir, seeded,
};
use super::google_drive_uploads::multipart_body;
use crate::fake_http_transport::FakeHttpTransport;
use crate::http_transport::UPLOAD_CHUNK_BYTES;
use crate::{CloudStorageProvider, Rev};

#[test]
fn multipart_create_has_metadata_then_media() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"F","headRevisionId":"r1"}"#);
    drive.write("index.json", b"\x00media\xff", None).unwrap();
    let upload = http.requests().pop().unwrap();
    assert_eq!(upload.method, HttpMethod::Post);
    assert!(upload
        .url
        .contains("/upload/drive/v3/files?uploadType=multipart&fields="));
    let content_type = upload.header("content-type").unwrap();
    let boundary = content_type
        .strip_prefix("multipart/related; boundary=")
        .unwrap();
    assert!(boundary.len() > 20, "random boundary");
    let want = multipart_body(
        boundary,
        r#"{"name":"index.json","parents":["v1"]}"#,
        b"\x00media\xff",
    );
    assert_eq!(upload.body, want);
    let text = upload.body_text();
    assert!(text.starts_with(&format!("--{boundary}\r\nContent-Type: application/json")));
    assert!(text.ends_with(&format!("\r\n--{boundary}--\r\n")));
    assert!(text.contains("Content-Type: application/octet-stream\r\n\r\n"));
}

#[test]
fn nested_blob_path_creates_each_new_folder_once() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let chain = [
        "games",
        "sys",
        "sha",
        "blobs",
        "1-abcdef12",
        "PSP",
        "SAVEDATA",
        "ULUS1",
    ];
    for (n, _) in chain.iter().enumerate() {
        http.push_json(200, &files(""));
        http.push_json(200, &format!(r#"{{"id":"d{n}"}}"#));
    }
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"f1","headRevisionId":"r"}"#);
    drive
        .write(
            "games/sys/sha/blobs/1-abcdef12/PSP/SAVEDATA/ULUS1/DATA.BIN",
            b"a",
            None,
        )
        .unwrap();
    let creates = |http: &FakeHttpTransport| {
        http.requests()
            .iter()
            .filter(|r| is_folder_create(r))
            .count()
    };
    assert_eq!(creates(&http), chain.len());
    assert!(http.requests()[3]
        .body_text()
        .contains(r#""parents":["d0"]"#));

    // A sibling unit file: only the new last folder is created.
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"d8"}"#);
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"f2","headRevisionId":"r"}"#);
    let before = http.requests().len();
    drive
        .write(
            "games/sys/sha/blobs/1-abcdef12/PSP/SAVEDATA/ULUS2/DATA.BIN",
            b"b",
            None,
        )
        .unwrap();
    assert_eq!(creates(&http), chain.len() + 1);
    assert_eq!(http.requests().len() - before, 4);
    assert!(
        q_of(&http.requests()[before]).contains("'d6' in parents"),
        "SAVEDATA from the cache"
    );

    // Same folder again: no lookups, no creates.
    http.push_json(200, &files(""));
    http.push_json(200, r#"{"id":"f3","headRevisionId":"r"}"#);
    drive
        .write(
            "games/sys/sha/blobs/1-abcdef12/PSP/SAVEDATA/ULUS2/ICON0.PNG",
            b"c",
            None,
        )
        .unwrap();
    assert_eq!(creates(&http), chain.len() + 1);
    assert_eq!(http.pending(), 0);
}

#[test]
fn resumable_upload_resumes_from_range_after_a_503() {
    let http = Arc::new(FakeHttpTransport::new());
    let drive = seeded(&http);
    let chunk = UPLOAD_CHUNK_BYTES;
    let len = 2 * chunk + 10;
    let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    script_blob_dir(&http);
    http.push_reply(reply(
        200,
        &[("location", "https://upload.example/session-1")],
        "",
    ));
    let acked = |last: u64| format!("bytes=0-{last}");
    http.push_reply(reply(308, &[("range", &acked(chunk - 1))], ""));
    http.push_reply(reply(503, &[("retry-after", "0")], ""));
    http.push_reply(reply(308, &[("range", &acked(chunk + 999))], ""));
    http.push_reply(reply(308, &[("range", &acked(2 * chunk - 1))], ""));
    http.push_json(200, r#"{"id":"big","headRevisionId":"r9"}"#);
    let rev = drive
        .write_large("blobs/1-x/big.bin", &mut data.as_slice(), len)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rev, Rev("r9".into()));

    let requests = http.requests();
    let init = &requests[5]; // blobs + 1-x looked up / created, file looked up
    assert!(init.url.contains("uploadType=resumable"));
    assert_eq!(
        init.header("x-upload-content-length"),
        Some(len.to_string().as_str())
    );
    let puts: Vec<_> = requests
        .iter()
        .filter(|r| r.method == HttpMethod::Put)
        .collect();
    let ranges: Vec<_> = puts
        .iter()
        .map(|r| r.header("content-range").unwrap().to_string())
        .collect();
    assert_eq!(
        ranges,
        [
            format!("bytes 0-{}/{len}", chunk - 1),
            format!("bytes {chunk}-{}/{len}", 2 * chunk - 1),
            format!("bytes */{len}"),
            format!("bytes {}-{}/{len}", chunk + 1000, 2 * chunk - 1),
            format!("bytes {}-{}/{len}", 2 * chunk, len - 1),
        ]
    );
    assert!(puts.iter().all(
        |r| r.header("authorization").is_none() && r.url == "https://upload.example/session-1"
    ));
    assert_eq!(
        puts[3].body,
        data[(chunk + 1000) as usize..(2 * chunk) as usize]
    );
    assert_eq!(puts[4].body, data[(2 * chunk) as usize..]);
    assert!(puts[2].body.is_empty());
    let chunk_timeout = Some(std::time::Duration::from_secs(80));
    assert_eq!(puts[0].timeout, chunk_timeout, "sized to the 5 MiB chunk");
}
