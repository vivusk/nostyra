//! Scripted [`HttpTransport`] for tests (no network): replies are queued in order,
//! every request is recorded (method, URL, headers, body bytes) for assertions.
//! Test-support only.

use std::collections::VecDeque;
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

use http_download_util::{HttpMethod, HttpReply, HttpRequest, RequestBody};

use crate::http_transport::HttpTransport;
use crate::CloudError;

/// One request as the transport saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Whole-call timeout the caller asked for.
    pub timeout: Option<Duration>,
}

impl RecordedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// A queued answer.
#[derive(Debug, Clone)]
pub struct ScriptedReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// In-memory transport answering from a script.
#[derive(Default)]
pub struct FakeHttpTransport {
    script: Mutex<VecDeque<Result<ScriptedReply, CloudError>>>,
    recorded: Mutex<Vec<RecordedRequest>>,
}

impl FakeHttpTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a JSON reply.
    pub fn push_json(&self, status: u16, json: &str) {
        self.push_reply(ScriptedReply {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: json.as_bytes().to_vec(),
        });
    }

    pub fn push_reply(&self, reply: ScriptedReply) {
        lock(&self.script).push_back(Ok(reply));
    }

    /// Queues a transport failure (e.g. `CloudError::Offline`).
    pub fn push_error(&self, error: CloudError) {
        lock(&self.script).push_back(Err(error));
    }

    /// Every request sent so far, oldest first.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        lock(&self.recorded).clone()
    }

    /// Replies not consumed yet.
    pub fn pending(&self) -> usize {
        lock(&self.script).len()
    }
}

impl HttpTransport for FakeHttpTransport {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, CloudError> {
        let body = match request.body {
            RequestBody::Empty => Vec::new(),
            RequestBody::Bytes(bytes) => bytes.to_vec(),
            RequestBody::Reader { reader, len } => {
                let mut body = Vec::new();
                reader
                    .take(len)
                    .read_to_end(&mut body)
                    .map_err(|err| CloudError::Other(err.to_string()))?;
                body
            }
        };
        lock(&self.recorded).push(RecordedRequest {
            method: request.method,
            url: request.url.to_string(),
            headers: request
                .headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body,
            timeout: request.timeout,
        });
        let next = lock(&self.script)
            .pop_front()
            .unwrap_or_else(|| Err(CloudError::Other("no scripted reply".into())))?;
        Ok(HttpReply {
            status: next.status,
            headers: next.headers,
            body: Box::new(std::io::Cursor::new(next.body)),
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
