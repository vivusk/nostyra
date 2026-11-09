use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::sync::Mutex;
use std::thread::JoinHandle;

use percent_encoding::percent_decode_str;

use super::*;
use crate::fake_http_transport::FakeHttpTransport;
use crate::oauth_endpoints::tests::endpoints;
use crate::token_store_files::{load_account, save_signed_in_account, StoredAccount};
use crate::OAuthProvider;

/// The spawned browser thread, filled once the authorize URL is opened.
type BrowserThread = std::sync::Arc<Mutex<Option<JoinHandle<()>>>>;

fn query_param(url: &str, key: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    let value = query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .unwrap();
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

/// Plays the browser: follows the authorize URL's redirect with `answer(state)`.
fn browser(answer: fn(&str) -> String) -> (impl Fn(&str), BrowserThread) {
    let handle = std::sync::Arc::new(Mutex::new(None));
    let slot = handle.clone();
    let open = move |url: &str| {
        let redirect = query_param(url, "redirect_uri");
        let port: u16 = redirect
            .trim_start_matches("http://localhost:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let target = answer(&query_param(url, "state"));
        *slot.lock().unwrap() = Some(std::thread::spawn(move || {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            write!(stream, "GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let _ = stream.read_to_end(&mut Vec::new());
        }));
    };
    (open, handle)
}

fn identity(token: &AccessToken) -> Result<AccountIdentity, CloudError> {
    assert_eq!(token.value.expose(), "a1");
    Ok(AccountIdentity {
        account_id: "id-1".into(),
        account_label: "player@example.com".into(),
    })
}

#[test]
fn full_sign_in_through_the_loopback_redirect() {
    let http = FakeHttpTransport::new();
    http.push_json(
        200,
        r#"{"access_token":"a1","expires_in":3600,"refresh_token":"1//r"}"#,
    );
    let (open, handle) = browser(|state| format!("/?code=c-1&state={state}"));
    let cancel = AtomicBool::new(false);
    let signed = sign_in(&http, &endpoints(), open, identity, &cancel).unwrap();
    handle.lock().unwrap().take().unwrap().join().unwrap();
    assert_eq!(signed.refresh_token.expose(), "1//r");
    assert_eq!(signed.account.account_label, "player@example.com");
    assert_eq!(signed.scopes, endpoints().scopes);
    let body = http.requests()[0].body_text();
    assert!(body.contains("code=c-1") && body.contains("code_verifier="));
    assert!(!format!("{signed:?}").contains("1//r"));
}

#[test]
fn denied_consent_is_cancelled_and_missing_refresh_token_refused() {
    let http = FakeHttpTransport::new();
    let (open, _) = browser(|state| format!("/?error=access_denied&state={state}"));
    let cancel = AtomicBool::new(false);
    let result = sign_in(&http, &endpoints(), open, identity, &cancel);
    assert_eq!(result.unwrap_err(), SignInError::Cancelled);
    assert!(http.requests().is_empty());

    http.push_json(200, r#"{"access_token":"a1","expires_in":3600}"#);
    let (open, _) = browser(|state| format!("/?code=c&state={state}"));
    let result = sign_in(&http, &endpoints(), open, identity, &cancel);
    assert!(matches!(result, Err(SignInError::Refused(_))));
}

#[test]
fn sign_in_times_out_without_a_redirect() {
    let http = FakeHttpTransport::new();
    let cancel = AtomicBool::new(false);
    let result = sign_in_within(
        &http,
        &endpoints(),
        |_| {},
        identity,
        &cancel,
        Duration::from_millis(300),
    );
    assert_eq!(result.unwrap_err(), SignInError::TimedOut);
}

#[test]
fn sign_out_revokes_best_effort_and_deletes_files() {
    let dir = tempfile::tempdir().unwrap();
    let account = StoredAccount {
        provider: OAuthProvider::GoogleDrive,
        account_id: "id".into(),
        account_label: "label".into(),
        scopes: vec![],
        signed_in_at: 0,
    };
    save_signed_in_account(dir.path(), &account, &SecretString::new("1//r")).unwrap();
    let mut google = endpoints();
    google.revoke = TokenRevoke::FormToken {
        url: "https://auth.example/revoke".into(),
    };
    let http = FakeHttpTransport::new();
    http.push_error(CloudError::Offline); // revoke fails: files still go
    sign_out(dir.path(), &http, &google).unwrap();
    assert_eq!(load_account(dir.path()).unwrap(), None);
    let request = &http.requests()[0];
    assert_eq!(request.url, "https://auth.example/revoke");
    assert_eq!(request.body_text(), "token=1%2F%2Fr");

    // Dropbox style: refresh, then bearer POST.
    let mut dropbox = endpoints();
    dropbox.revoke = TokenRevoke::BearerPost {
        url: "https://api.example/revoke".into(),
    };
    let http = FakeHttpTransport::new();
    http.push_json(200, r#"{"access_token":"a9","expires_in":3600}"#);
    http.push_json(200, "null");
    revoke(&http, &dropbox, &SecretString::new("r")).unwrap();
    assert_eq!(
        http.requests()[1].header("authorization"),
        Some("Bearer a9")
    );
}
