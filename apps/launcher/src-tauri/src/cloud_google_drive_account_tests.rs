//! Google Drive wiring over a scripted transport: browser sign-in through the real
//! loopback listener → stored account + selected provider → a ready, cached account
//! whose provider refreshes with the stored token; the not-ready states.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use cloud_sync::fake_http_transport::FakeHttpTransport;
use cloud_sync::{refresh_token_file, ClientCredentials, SecretString};
use config_layers::{CloudConfig, CloudProviderId};

use super::*;
use crate::cloud_provider_factory::{cloud_config, current_account_with};
use crate::cloud_sync_state::CloudSyncState;

fn credentials() -> ClientCredentials {
    ClientCredentials {
        client_id: "cid.apps.googleusercontent.com".into(),
        client_secret: Some(SecretString::new("GOCSPX-test")),
    }
}

fn google_config() -> CloudConfig {
    CloudConfig {
        provider: CloudProviderId::GoogleDrive,
        ..CloudConfig::default()
    }
}

fn param(url: &str, key: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    let value = query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .unwrap();
    // Only `:` and `/` are encoded in the values read here (state is base64url).
    value.replace("%3A", ":").replace("%2F", "/")
}

/// Signs in as `me@example.com` with refresh token `1//r` (scripted Google replies).
fn signed_in(data_root: &Path) -> Arc<FakeHttpTransport> {
    let http = Arc::new(FakeHttpTransport::new());
    http.push_json(
        200,
        r#"{"access_token":"a1","expires_in":3599,"refresh_token":"1//r","scope":"https://www.googleapis.com/auth/drive.file"}"#,
    );
    http.push_json(
        200,
        r#"{"user":{"displayName":"Me","emailAddress":"me@example.com","permissionId":"perm-1"}}"#,
    );
    let browser = Mutex::new(None);
    let open = |url: &str| {
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert_eq!(param(url, "access_type"), "offline");
        assert_eq!(param(url, "prompt"), "consent");
        let redirect = param(url, "redirect_uri");
        let port: u16 = redirect
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let target = format!("/?code=c-1&state={}", param(url, "state"));
        *browser.lock().unwrap() = Some(std::thread::spawn(move || {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            write!(stream, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            let _ = stream.read_to_end(&mut Vec::new());
        }));
    };
    let cancel = AtomicBool::new(false);
    let signed = google_sign_in(http.as_ref(), credentials(), open, &cancel).unwrap();
    browser.lock().unwrap().take().unwrap().join().unwrap();
    assert_eq!(signed.account.account_id, "perm-1");
    store_google_sign_in(data_root, &signed).unwrap();
    let requests = http.requests();
    assert!(requests[0]
        .body_text()
        .contains("client_secret=GOCSPX-test"));
    assert_eq!(requests[1].header("authorization"), Some("Bearer a1"));
    http
}

#[test]
fn sign_in_stores_the_account_and_selects_google_drive() {
    let data = tempfile::tempdir().unwrap();
    signed_in(data.path());
    assert_eq!(
        cloud_config(data.path()).provider,
        CloudProviderId::GoogleDrive
    );
    let cloud = CloudSyncState::default();
    let first = current_account_with(
        data.path(),
        &cloud,
        &google_config(),
        || Some(credentials()),
    );
    let AccountState::Ready(first) = first else {
        panic!("not ready: {first:?}");
    };
    assert_eq!(first.label, "me@example.com");
    assert!(first.account_key.starts_with("google_drive-"));
    let again = current_account_with(
        data.path(),
        &cloud,
        &google_config(),
        || Some(credentials()),
    );
    let AccountState::Ready(again) = again else {
        panic!("not ready");
    };
    assert!(
        Arc::ptr_eq(&first.provider, &again.provider),
        "cached provider reused"
    );
}

#[test]
fn ready_provider_refreshes_with_the_stored_token() {
    let data = tempfile::tempdir().unwrap();
    signed_in(data.path());
    let http = Arc::new(FakeHttpTransport::new());
    let AccountState::Ready(account) =
        google_drive_account(data.path(), credentials(), http.clone())
    else {
        panic!("not ready");
    };
    http.push_json(200, r#"{"access_token":"a2","expires_in":3599}"#);
    http.push_json(200, r#"{"files":[]}"#); // no "Nostyra" folder yet
    assert_eq!(
        account.provider.list("").unwrap_err(),
        cloud_sync::CloudError::NotFound
    );
    let requests = http.requests();
    assert!(requests[0].body_text().contains("refresh_token=1%2F%2Fr"));
    assert_eq!(requests[1].header("authorization"), Some("Bearer a2"));
}

#[test]
fn pre_play_account_holds_drive_requests_to_the_deadline() {
    let data = tempfile::tempdir().unwrap();
    signed_in(data.path());
    let http = Arc::new(FakeHttpTransport::new());
    let AccountState::Ready(account) =
        google_drive_account(data.path(), credentials(), http.clone())
    else {
        panic!("not ready");
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let pre_play = crate::cloud_sync_jobs::within_deadline(&account, deadline);
    assert_eq!(pre_play.account_key, account.account_key);
    http.push_json(200, r#"{"access_token":"a2","expires_in":3599}"#);
    http.push_json(200, r#"{"files":[]}"#);
    assert!(pre_play.provider.list("").is_err());
    let short = cloud_sync::http_transport::PRE_PLAY_REQUEST_TIMEOUT;
    let requests = http.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|r| r.timeout.is_some_and(|t| t <= short)),
        "token refresh and the Drive call both pre-play bounded"
    );
}

#[test]
fn missing_client_id_account_or_token_is_not_ready() {
    let data = tempfile::tempdir().unwrap();
    let cloud = CloudSyncState::default();
    let state = |google: Option<ClientCredentials>| {
        current_account_with(data.path(), &cloud, &google_config(), || google)
    };
    assert!(matches!(
        state(None),
        AccountState::Unavailable(CloudProviderId::GoogleDrive)
    ));
    assert!(matches!(
        state(Some(credentials())),
        AccountState::NeedsSignIn(GOOGLE_SIGN_IN_NEEDED)
    ));
    signed_in(data.path());
    assert!(matches!(state(Some(credentials())), AccountState::Ready(_)));
    std::fs::remove_file(refresh_token_file(data.path())).unwrap();
    *crate::app_state::lock(&cloud.account) = None; // as after sign-out / restart
    assert!(matches!(
        state(Some(credentials())),
        AccountState::NeedsSignIn(SIGN_IN_AGAIN)
    ));
}
