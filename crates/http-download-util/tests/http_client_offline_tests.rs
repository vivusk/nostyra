//! Client guards that must hold without any network: the offline switch and
//! the HTTPS-only rule both fail before a socket is opened.

use http_download_util::{build_http_client, DownloadError};

#[test]
fn offline_client_fails_transient_without_network() {
    let client = build_http_client("Nostyra-test", true);
    assert!(client.is_offline());
    // An unroutable host would hang or fail differently if a socket were opened.
    let result = client.get("https://nostyra.invalid/x.png", &[]);
    assert!(matches!(result, Err(DownloadError::Transient(reason)) if reason == "offline"));
}

#[test]
fn plain_http_is_refused_before_io() {
    let client = build_http_client("Nostyra-test", false);
    let result = client.get("http://nostyra.invalid/x.png", &[]);
    assert!(matches!(result, Err(DownloadError::Permanent(_))));
}

#[test]
fn offline_form_post_fails_transient_without_network() {
    let client = build_http_client("Nostyra-test", true);
    let result = client.post_form("https://nostyra.invalid/dorequest.php", "r=login2", 1024);
    assert!(matches!(result, Err(DownloadError::Transient(reason)) if reason == "offline"));
}

#[test]
fn plain_http_form_post_is_refused_before_io() {
    let client = build_http_client("Nostyra-test", false);
    let result = client.post_form("http://nostyra.invalid/dorequest.php", "r=login2", 1024);
    assert!(matches!(result, Err(DownloadError::Permanent(_))));
}

#[test]
fn generic_request_refuses_every_method_offline_and_plain_http() {
    use http_download_util::{HttpMethod, HttpRequest, RequestBody};
    let offline = build_http_client("Nostyra-test", true);
    let online = build_http_client("Nostyra-test", false);
    for method in [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ] {
        let request = |url| HttpRequest {
            method,
            url,
            headers: &[("Authorization", "Bearer x")],
            body: RequestBody::Bytes(b"{}"),
            connect_timeout: None,
            timeout: None,
        };
        let result = offline.request(request("https://nostyra.invalid/api"));
        assert!(
            matches!(result, Err(DownloadError::Transient(ref reason)) if reason == "offline"),
            "{method:?}"
        );
        let result = online.request(request("http://nostyra.invalid/api"));
        assert!(
            matches!(result, Err(DownloadError::Permanent(_))),
            "{method:?}"
        );
    }
}
