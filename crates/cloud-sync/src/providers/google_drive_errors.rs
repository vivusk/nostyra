//! Drive v3 reply → [`CloudError`] and the `q` string escaping. Drive puts the
//! reason in `{"error":{"errors":[{"reason":…}]}}`: 403 `userRateLimitExceeded` /
//! `rateLimitExceeded` (and 429) = throttled, 403 `storageQuotaExceeded` = full,
//! 401 (after the session's one refresh) = sign in again, 5xx = transient.

use http_download_util::HttpReply;
use serde::Deserialize;

use crate::http_transport::{read_body, retry_after, status_error};
use crate::CloudError;

/// Error bodies are tiny; anything bigger is cut (only the reason matters).
const MAX_ERROR_BODY_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    #[serde(default)]
    errors: Vec<ErrorItem>,
}

#[derive(Deserialize)]
struct ErrorItem {
    #[serde(default)]
    reason: String,
}

/// Classifies a non-2xx Drive reply (consumes the body for the reason).
pub(crate) fn drive_error(reply: HttpReply) -> CloudError {
    let status = reply.status;
    let wait = retry_after(&reply);
    let generic = status_error(&reply);
    if !matches!(status, 400 | 403 | 429) {
        return generic;
    }
    let reasons = read_body(reply, MAX_ERROR_BODY_BYTES)
        .ok()
        .and_then(|body| serde_json::from_slice::<ErrorBody>(&body).ok())
        .map(|body| body.error.errors)
        .unwrap_or_default();
    let has = |wanted: &[&str]| {
        reasons
            .iter()
            .any(|item| wanted.contains(&item.reason.as_str()))
    };
    if status == 429 || has(&["userRateLimitExceeded", "rateLimitExceeded"]) {
        return CloudError::RateLimited { retry_after: wait };
    }
    if has(&["storageQuotaExceeded"]) {
        return CloudError::QuotaFull;
    }
    match reasons.first() {
        Some(item) => {
            CloudError::Other(format!("drive refused ({status}): {}", safe(&item.reason)))
        }
        None => generic,
    }
}

/// Drive refused a recursive folder delete because of one child (`drive.file`).
pub(crate) fn is_child_refusal(err: &CloudError) -> bool {
    matches!(err, CloudError::Other(msg) if msg.ends_with(": appNotAuthorizedToChild"))
}

/// A value inside a single-quoted `q` literal: `\` → `\\`, `'` → `\'`.
pub(crate) fn escape_q(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Keeps only reason-like characters (never echoes arbitrary server text).
fn safe(reason: &str) -> String {
    reason
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> HttpReply {
        HttpReply {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Box::new(std::io::Cursor::new(body.as_bytes().to_vec())),
        }
    }

    fn reason(status: u16, reason: &str) -> HttpReply {
        let body = format!(r#"{{"error":{{"code":{status},"errors":[{{"reason":"{reason}"}}]}}}}"#);
        reply(status, &[], &body)
    }

    #[test]
    fn maps_drive_reasons() {
        assert_eq!(
            drive_error(reason(403, "storageQuotaExceeded")),
            CloudError::QuotaFull
        );
        assert_eq!(
            drive_error(reason(403, "userRateLimitExceeded")),
            CloudError::RateLimited { retry_after: None }
        );
        assert_eq!(
            drive_error(reply(429, &[("Retry-After", "7")], "")),
            CloudError::RateLimited {
                retry_after: Some(std::time::Duration::from_secs(7))
            }
        );
        assert_eq!(
            drive_error(reason(403, "insufficientFilePermissions")),
            CloudError::Other("drive refused (403): insufficientFilePermissions".into())
        );
        assert_eq!(drive_error(reply(401, &[], "")), CloudError::AuthExpired);
        assert_eq!(drive_error(reply(404, &[], "")), CloudError::NotFound);
        assert_eq!(drive_error(reply(500, &[], "")), CloudError::Offline);
    }

    #[test]
    fn escapes_quotes_and_backslashes() {
        assert_eq!(escape_q(r"it's a\b"), r"it\'s a\\b");
    }
}
