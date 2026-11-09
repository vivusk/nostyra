//! An in-memory Drive v3 for executor tests: answers the exact requests the Google
//! provider sends (`files.list` with our `q` shapes, folder create, multipart and
//! resumable uploads, ranged media download, recursive delete). Like real Drive it has no
//! If-Match and allows same-name duplicates.

use std::collections::HashMap;
use std::sync::Mutex;

use http_download_util::{HttpMethod, HttpReply, HttpRequest, RequestBody};
use percent_encoding::percent_decode_str;
use serde_json::Value;

use super::google_drive_fake_server_store::split_multipart;

use crate::http_transport::HttpTransport;
use crate::CloudError;

#[derive(Clone)]
pub(super) struct Node {
    pub(super) name: String,
    pub(super) parent: Option<String>,
    pub(super) folder: bool,
    pub(super) marker: Option<String>,
    pub(super) data: Vec<u8>,
    pub(super) rev: u64,
}

pub(super) struct Session {
    pub(super) target: Value,
    pub(super) file: Option<String>,
    pub(super) data: Vec<u8>,
    pub(super) len: usize,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) nodes: HashMap<String, Node>,
    pub(super) order: Vec<String>,
    pub(super) sessions: HashMap<String, Session>,
    pub(super) next: u64,
}

/// The fake Drive (shared by every simulated PC's provider).
#[derive(Default)]
pub(super) struct FakeDriveServer {
    state: Mutex<State>,
}

impl HttpTransport for FakeDriveServer {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, CloudError> {
        let body = match request.body {
            RequestBody::Empty => Vec::new(),
            RequestBody::Bytes(bytes) => bytes.to_vec(),
            RequestBody::Reader { reader, .. } => {
                let mut all = Vec::new();
                reader
                    .read_to_end(&mut all)
                    .map_err(|e| CloudError::Other(e.to_string()))?;
                all
            }
        };
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.to_string())
        };
        let (path, query) = request.url.split_once('?').unwrap_or((request.url, ""));
        let params: HashMap<String, String> = query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| {
                (
                    k.to_string(),
                    percent_decode_str(v).decode_utf8_lossy().into_owned(),
                )
            })
            .collect();
        let mut state = self.state.lock().unwrap();
        let (status, headers, out) = state.handle(request.method, path, &params, &body, header);
        Ok(HttpReply {
            status,
            headers,
            body: Box::new(std::io::Cursor::new(out)),
        })
    }
}

type Answer = (u16, Vec<(String, String)>, Vec<u8>);

fn json_answer(value: Value) -> Answer {
    (200, Vec::new(), value.to_string().into_bytes())
}

impl State {
    fn handle(
        &mut self,
        method: HttpMethod,
        path: &str,
        params: &HashMap<String, String>,
        body: &[u8],
        header: impl Fn(&str) -> Option<String>,
    ) -> Answer {
        let files = "https://www.googleapis.com/drive/v3/files";
        let upload = "https://www.googleapis.com/upload/drive/v3/files";
        let upload_type = params.get("uploadType").map(String::as_str);
        match (method, path) {
            (HttpMethod::Get, p) if p == files => json_answer(self.list(&params["q"])),
            (HttpMethod::Post, p) if p == files => {
                let meta: Value = serde_json::from_slice(body).unwrap();
                json_answer(self.create(&meta, Vec::new(), true))
            }
            (HttpMethod::Get, p) if p.starts_with(files) => {
                let Some(node) = self.nodes.get(&p[files.len() + 1..]) else {
                    return (404, Vec::new(), Vec::new());
                };
                match header("range").and_then(|r| r.strip_prefix("bytes=").map(str::to_string)) {
                    Some(range) => {
                        let (first, last) = range.split_once('-').unwrap();
                        let (first, last): (usize, usize) =
                            (first.parse().unwrap(), last.parse().unwrap());
                        (206, Vec::new(), node.data[first..=last].to_vec())
                    }
                    None => (200, Vec::new(), node.data.clone()),
                }
            }
            (HttpMethod::Delete, p) => {
                let id = &p[files.len() + 1..];
                if !self.nodes.contains_key(id) {
                    return (404, Vec::new(), Vec::new());
                }
                self.remove_tree(id);
                (204, Vec::new(), Vec::new())
            }
            (HttpMethod::Post | HttpMethod::Patch, p) if p.starts_with(upload) => {
                let file = p
                    .strip_prefix(upload)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .map(str::to_string);
                if upload_type == Some("resumable") {
                    let len = header("x-upload-content-length").unwrap().parse().unwrap();
                    let target = serde_json::from_slice(body).unwrap();
                    self.next += 1;
                    let url = format!("https://upload.fake/session/{}", self.next);
                    self.sessions.insert(
                        url.clone(),
                        Session {
                            target,
                            file,
                            data: Vec::new(),
                            len,
                        },
                    );
                    return (200, vec![("location".into(), url)], Vec::new());
                }
                let boundary = header("content-type")
                    .unwrap()
                    .split("boundary=")
                    .nth(1)
                    .unwrap()
                    .to_string();
                let (meta, media) = split_multipart(body, &boundary);
                json_answer(self.store(file, &meta, media))
            }
            (HttpMethod::Put, url) => {
                let range = header("content-range").unwrap();
                let session = self.sessions.get_mut(url).unwrap();
                if !range.starts_with("bytes */") {
                    let start: usize = range[6..].split('-').next().unwrap().parse().unwrap();
                    session.data.truncate(start);
                    session.data.extend_from_slice(body);
                }
                if session.data.len() < session.len {
                    let acked = format!("bytes=0-{}", session.data.len() - 1);
                    return (308, vec![("range".into(), acked)], Vec::new());
                }
                let session = self.sessions.remove(url).unwrap();
                json_answer(self.store(session.file, &session.target, session.data))
            }
            _ => (400, Vec::new(), Vec::new()),
        }
    }
}
