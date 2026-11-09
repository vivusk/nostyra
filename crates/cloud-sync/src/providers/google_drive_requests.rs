//! One authorized Drive request: bearer from the session (one refresh after a 401),
//! per-request timeout from the provider's budget (sized to the media it moves), retry on transient failures
//! (non-idempotent creates only after an explicit `Retry-After` refusal), non-2xx
//! mapped by [`drive_error`]. Plus `files.list` and the JSON reply shapes.

use http_download_util::{HttpMethod, HttpReply, HttpRequest, RequestBody, RetryPolicy};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::google_drive_errors::{drive_error, escape_q, is_child_refusal};
use super::google_drive_provider::GoogleDriveProvider;
use crate::http_transport::{read_body, with_retry, CONNECT_TIMEOUT, MAX_JSON_REPLY_BYTES};
use crate::oauth_endpoints::form_encode;
use crate::CloudError;

/// Drive v3 metadata API.
pub(crate) const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
/// Drive v3 upload API (multipart / resumable).
pub(crate) const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
pub(crate) const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
pub(crate) const JSON_TYPE: &str = "application/json; charset=UTF-8";
/// Fields of a looked-up file (rev for reads / the write pre-check).
pub(crate) const FILE_FIELDS: &str = "id,name,mimeType,size,headRevisionId,md5Checksum";

/// One request to send.
pub(crate) struct DriveCall<'a> {
    pub method: HttpMethod,
    pub url: String,
    pub content_type: Option<&'a str>,
    pub extra_headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
    /// Media bytes the reply carries (ranged download): sizes the timeout.
    pub reply_bytes: u64,
    /// False: only retried after a `Retry-After` refusal (creates must not run twice).
    pub idempotent: bool,
}

impl<'a> DriveCall<'a> {
    pub fn get(url: String) -> Self {
        Self::new(HttpMethod::Get, url, None, &[], true)
    }

    pub fn new(
        method: HttpMethod,
        url: String,
        content_type: Option<&'a str>,
        body: &'a [u8],
        idempotent: bool,
    ) -> Self {
        Self {
            method,
            url,
            content_type,
            extra_headers: &[],
            body,
            reply_bytes: 0,
            idempotent,
        }
    }
}

/// A Drive file as `files.list` / `files.create` / uploads return it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveFile {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    pub head_revision_id: Option<String>,
    /// Bytes (Drive sends int64 as a string); none for folders.
    #[serde(default)]
    size: Option<String>,
}

impl DriveFile {
    pub fn size(&self) -> Option<u64> {
        self.size.as_deref().and_then(|size| size.parse().ok())
    }

    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
    next_page_token: Option<String>,
}

impl GoogleDriveProvider {
    /// Sends `call`; the 2xx reply or the mapped error.
    pub(crate) fn send(&self, call: &DriveCall<'_>) -> Result<HttpReply, CloudError> {
        let budget = self.budget;
        let media_bytes = call.body.len() as u64 + call.reply_bytes;
        let policy = RetryPolicy {
            idempotent: call.idempotent,
            ..budget.retry_policy()
        };
        let transport = self.session.transport();
        with_retry(transport, policy, || {
            let reply = self.session.authorized(budget, |auth| {
                let mut headers = vec![("Authorization", auth)];
                if let Some(content_type) = call.content_type {
                    headers.push(("Content-Type", content_type));
                }
                headers.extend_from_slice(call.extra_headers);
                transport.send(HttpRequest {
                    method: call.method,
                    url: &call.url,
                    headers: &headers,
                    body: if call.body.is_empty() {
                        RequestBody::Empty
                    } else {
                        RequestBody::Bytes(call.body)
                    },
                    connect_timeout: Some(CONNECT_TIMEOUT),
                    timeout: Some(budget.transfer_timeout(media_bytes)?),
                })
            })?;
            match reply.status {
                200..=299 => Ok(reply),
                _ => Err(drive_error(reply)),
            }
        })
    }

    /// Sends `call` and parses the JSON reply.
    pub(crate) fn send_json<T: DeserializeOwned>(
        &self,
        call: &DriveCall<'_>,
    ) -> Result<T, CloudError> {
        let body = read_body(self.send(call)?, MAX_JSON_REPLY_BYTES)?;
        serde_json::from_slice(&body)
            .map_err(|_| CloudError::Other("unreadable drive reply".into()))
    }

    /// Every file matching `q` (all pages), ordered by `order_by`.
    pub(crate) fn query_files(
        &self,
        q: &str,
        order_by: &str,
    ) -> Result<Vec<DriveFile>, CloudError> {
        let fields = format!("nextPageToken,files({FILE_FIELDS})");
        let mut files = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut pairs = vec![
                ("q", q),
                ("spaces", "drive"),
                ("pageSize", "1000"),
                ("orderBy", order_by),
                ("fields", fields.as_str()),
            ];
            if let Some(token) = page_token.as_deref() {
                pairs.push(("pageToken", token));
            }
            let url = format!("{DRIVE_API}/files?{}", form_encode(&pairs));
            let page: FileList = self.send_json(&DriveCall::get(url))?;
            files.extend(page.files);
            match page.next_page_token.filter(|token| !token.is_empty()) {
                Some(token) => page_token = Some(token),
                None => return Ok(files),
            }
        }
    }

    /// `files.delete` (permanent; folders take their children along). Gone = ok.
    pub(crate) fn delete_id(&self, id: &str) -> Result<(), CloudError> {
        let url = format!("{DRIVE_API}/files/{}", crate::encode_component(id));
        match self.send(&DriveCall::new(HttpMethod::Delete, url, None, &[], true)) {
            Ok(_) | Err(CloudError::NotFound) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Deletes a folder with everything in it. Drive deletes children along, but
    /// under `drive.file` it can refuse (`appNotAuthorizedToChild`, seen live right
    /// after a sync wrote the tree) → children first, bottom-up, then the folder.
    pub(crate) fn delete_folder(&self, id: &str) -> Result<(), CloudError> {
        match self.delete_id(id) {
            Err(err) if is_child_refusal(&err) => {}
            other => return other,
        }
        let q = format!("'{}' in parents and trashed = false", escape_q(id));
        for child in self.query_files(&q, "name")? {
            match child.is_folder() {
                true => self.delete_folder(&child.id)?,
                false => self.delete_id(&child.id)?,
            }
        }
        self.delete_id(id)
    }
}
