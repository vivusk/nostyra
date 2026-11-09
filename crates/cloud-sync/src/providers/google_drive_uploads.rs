//! Drive uploads: where they go ([`UploadTarget`]: a new file in a folder or a new
//! version of a file) and small files as one multipart request (`files.create` /
//! PATCH update). Big files: `google_drive_resumable_upload`.

use http_download_util::HttpMethod;
use serde_json::json;

use super::google_drive_provider::GoogleDriveProvider;
use super::google_drive_requests::{DriveCall, DriveFile, DRIVE_UPLOAD, JSON_TYPE};
use crate::content_hash::to_hex;
use crate::{CloudError, Rev};

/// `fields` of an upload reply.
const UPLOAD_FIELDS: &str = "fields=id,name,mimeType,headRevisionId";

/// Where an upload goes: a new file in a folder or a new version of a file.
pub(crate) enum UploadTarget<'a> {
    Create { parent: &'a str, name: &'a str },
    Update { id: &'a str },
}

impl UploadTarget<'_> {
    pub(crate) fn method_and_url(&self, upload_type: &str) -> (HttpMethod, String) {
        match self {
            Self::Create { .. } => (
                HttpMethod::Post,
                format!("{DRIVE_UPLOAD}?uploadType={upload_type}&{UPLOAD_FIELDS}"),
            ),
            Self::Update { id } => (
                HttpMethod::Patch,
                format!(
                    "{DRIVE_UPLOAD}/{}?uploadType={upload_type}&{UPLOAD_FIELDS}",
                    crate::encode_component(id)
                ),
            ),
        }
    }

    pub(crate) fn metadata(&self) -> String {
        match self {
            Self::Create { parent, name } => json!({ "name": name, "parents": [parent] }),
            Self::Update { .. } => json!({}),
        }
        .to_string()
    }
}

/// `multipart/related` body: metadata JSON part, then the media part.
pub(crate) fn multipart_body(boundary: &str, metadata: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(data.len() + metadata.len() + 256);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Type: {JSON_TYPE}\r\n\r\n{metadata}\r\n\
             --{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// The new revision an upload reply names.
pub(crate) fn rev_of(file: &DriveFile) -> Result<Rev, CloudError> {
    file.head_revision_id
        .clone()
        .map(Rev)
        .ok_or_else(|| CloudError::Other("drive file without a revision".into()))
}

impl GoogleDriveProvider {
    /// One multipart request (files ≤ 4 MiB).
    pub(crate) fn upload_multipart(
        &self,
        target: &UploadTarget<'_>,
        data: &[u8],
    ) -> Result<Rev, CloudError> {
        let boundary = random_boundary()?;
        let body = multipart_body(&boundary, &target.metadata(), data);
        let content_type = format!("multipart/related; boundary={boundary}");
        let (method, url) = target.method_and_url("multipart");
        let idempotent = matches!(target, UploadTarget::Update { .. });
        let call = DriveCall::new(method, url, Some(&content_type), &body, idempotent);
        rev_of(&self.send_json::<DriveFile>(&call)?)
    }
}

fn random_boundary() -> Result<String, CloudError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|err| CloudError::Other(err.to_string()))?;
    Ok(format!("nostyra-{}", to_hex(&bytes)))
}
