use std::sync::{Arc, Mutex};

use http_download_util::{HttpMethod, HttpReply, HttpRequest, RequestBody};

use super::*;
use crate::fake_http_transport::{FakeHttpTransport, ScriptedReply};
use crate::http_transport::{HttpTransport, PRE_PLAY_REQUEST_TIMEOUT, REQUEST_TIMEOUT};
use crate::oauth_endpoints::tests::endpoints;
use crate::oauth_token_session::OAuthSession;

#[test]
fn exchange_sends_pkce_form_and_parses_tokens() {
    let http = FakeHttpTransport::new();
    http.push_json(
        200,
        r#"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"s1 s2","token_type":"Bearer"}"#,
    );
    let grant = exchange_code(
        &http,
        &endpoints(),
        &SecretString::new("4/code"),
        &SecretString::new("verifier-x"),
        "http://localhost:5000/",
    )
    .unwrap();
    assert_eq!(grant.access_token.value.expose(), "ya29.a");
    assert!(grant.access_token.is_fresh(Instant::now()));
    assert_eq!(grant.refresh_token.unwrap().expose(), "1//r");
    assert_eq!(grant.scope.as_deref(), Some("s1 s2"));
    let request = &http.requests()[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert_eq!(request.url, "https://auth.example/token");
    assert_eq!(
        request.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(
        request.body_text(),
        "grant_type=authorization_code&code=4%2Fcode\
         &redirect_uri=http%3A%2F%2Flocalhost%3A5000%2F&client_id=client%201\
         &code_verifier=verifier-x&client_secret=shh"
    );
}

#[test]
fn exchange_is_not_retried_after_a_server_error() {
    let http = FakeHttpTransport::new();
    http.push_json(502, "{}");
    http.push_json(200, r#"{"access_token":"never"}"#);
    let code = SecretString::new("c");
    let result = exchange_code(&http, &endpoints(), &code, &code, "http://x/");
    assert_eq!(result.unwrap_err(), CloudError::Offline);
    assert_eq!(http.pending(), 1);
}

#[test]
fn refresh_keeps_or_rotates_and_maps_invalid_grant() {
    let http = FakeHttpTransport::new();
    http.push_json(200, r#"{"access_token":"a2","expires_in":3600}"#);
    let grant = refresh_access_token(
        &http,
        &endpoints(),
        &SecretString::new("1//r"),
        RequestBudget::BACKGROUND,
    )
    .unwrap();
    assert!(grant.refresh_token.is_none());
    assert!(http.requests()[0]
        .body_text()
        .starts_with("grant_type=refresh_token&refresh_token=1%2F%2Fr&client_id="));

    http.push_json(
        400,
        r#"{"error":"invalid_grant","error_description":"revoked"}"#,
    );
    let result = refresh_access_token(
        &http,
        &endpoints(),
        &SecretString::new("1//r"),
        RequestBudget::BACKGROUND,
    );
    assert_eq!(result.unwrap_err(), CloudError::AuthExpired);

    http.push_json(401, r#"{"error":"invalid_client"}"#);
    let result = refresh_access_token(
        &http,
        &endpoints(),
        &SecretString::new("1//r"),
        RequestBudget::BACKGROUND,
    );
    assert_eq!(
        result.unwrap_err(),
        CloudError::Other("token endpoint refused: invalid_client".into())
    );
}

/// Session over a shared fake; returns (session, transport, rotated tokens seen).
fn new_session(
    access: Option<AccessToken>,
) -> (
    OAuthSession,
    Arc<FakeHttpTransport>,
    Arc<Mutex<Vec<String>>>,
) {
    let http = Arc::new(FakeHttpTransport::new());
    let rotated = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&rotated);
    let session = OAuthSession::new(
        endpoints(),
        http.clone(),
        SecretString::new("M.R3_old"),
        access,
        Box::new(move |token| {
            sink.lock().unwrap().push(token.expose().to_string());
            Ok(())
        }),
    );
    (session, http, rotated)
}

fn get(http: &dyn HttpTransport, bearer: &str) -> Result<HttpReply, CloudError> {
    http.send(HttpRequest {
        method: HttpMethod::Get,
        url: "https://api.example/me",
        headers: &[("Authorization", bearer)],
        body: RequestBody::Empty,
        connect_timeout: None,
        timeout: None,
    })
}

#[test]
fn microsoft_rotated_refresh_token_is_persisted_and_used() {
    let (session, http, rotated) = new_session(None);
    http.push_json(
        200,
        r#"{"access_token":"a1","expires_in":3600,"refresh_token":"M.R3_new"}"#,
    );
    assert_eq!(
        session
            .access_token(RequestBudget::BACKGROUND)
            .unwrap()
            .value
            .expose(),
        "a1"
    );
    assert_eq!(*rotated.lock().unwrap(), ["M.R3_new"]);
    assert_eq!(session.refresh_token().expose(), "M.R3_new");
    // Fresh token is reused without another refresh.
    session.access_token(RequestBudget::BACKGROUND).unwrap();
    assert_eq!(http.requests().len(), 1);
}

#[test]
fn early_refresh_inside_the_margin() {
    let stale = AccessToken {
        value: SecretString::new("old"),
        expires_at: Instant::now() + Duration::from_secs(30),
    };
    let (session, http, _) = new_session(Some(stale));
    http.push_json(200, r#"{"access_token":"a1","expires_in":3600}"#);
    assert_eq!(
        session
            .access_token(RequestBudget::BACKGROUND)
            .unwrap()
            .value
            .expose(),
        "a1"
    );
}

#[test]
fn a_401_refreshes_once_and_repeats_the_call() {
    let fresh = AccessToken {
        value: SecretString::new("a0"),
        expires_at: Instant::now() + Duration::from_secs(3600),
    };
    let (session, http, _) = new_session(Some(fresh.clone()));
    http.push_json(401, "{}");
    http.push_json(200, r#"{"access_token":"a1","expires_in":3600}"#);
    http.push_json(200, r#"{"id":"me"}"#);
    let reply = session
        .authorized(RequestBudget::BACKGROUND, |bearer| {
            get(http.as_ref(), bearer)
        })
        .unwrap();
    assert_eq!(reply.status, 200);
    let auth: Vec<_> = http
        .requests()
        .iter()
        .filter_map(|r| r.header("authorization").map(str::to_string))
        .collect();
    assert_eq!(auth, ["Bearer a0", "Bearer a1"]);

    // A second 401 is handed back, not refreshed again.
    let (session, http, _) = new_session(Some(fresh));
    http.push_json(401, "{}");
    http.push_json(200, r#"{"access_token":"a1","expires_in":3600}"#);
    http.push_json(401, "{}");
    let reply = session
        .authorized(RequestBudget::BACKGROUND, |bearer| {
            get(http.as_ref(), bearer)
        })
        .unwrap();
    assert_eq!((reply.status, http.pending()), (401, 0));
}

#[test]
fn no_token_in_debug_output() {
    let (session, _, _) = new_session(None);
    let token = AccessToken {
        value: SecretString::new("ya29.secret"),
        expires_at: Instant::now(),
    };
    let grant = TokenGrant {
        access_token: token.clone(),
        refresh_token: Some(SecretString::new("1//secret")),
        scope: None,
    };
    let text = format!("{session:?} {token:?} {grant:?} {:?}", endpoints());
    for value in ["ya29.secret", "1//secret", "M.R3_old", "\"shh\""] {
        assert!(!text.contains(value), "{value} leaked: {text}");
    }
}

#[test]
fn refresh_honours_the_pre_play_budget() {
    let http = FakeHttpTransport::new();
    let token = SecretString::new("1//r");
    // Timeouts: background 30 s; pre-play 10 s.
    http.push_json(200, r#"{"access_token":"a1"}"#);
    refresh_access_token(&http, &endpoints(), &token, RequestBudget::BACKGROUND).unwrap();
    http.push_json(200, r#"{"access_token":"a2"}"#);
    let far = RequestBudget::until(Instant::now() + Duration::from_secs(60));
    refresh_access_token(&http, &endpoints(), &token, far).unwrap();
    let timeouts: Vec<_> = http.requests().iter().map(|r| r.timeout).collect();
    assert_eq!(
        timeouts,
        [Some(REQUEST_TIMEOUT), Some(PRE_PLAY_REQUEST_TIMEOUT)]
    );

    // Network down, 2 s left: at most two tries, no sleep past the deadline, the
    // request timeout is clipped to the remaining budget.
    let http = FakeHttpTransport::new();
    for _ in 0..5 {
        http.push_error(CloudError::Offline);
    }
    let started = Instant::now();
    let budget = RequestBudget::until(started + Duration::from_secs(2));
    let result = refresh_access_token(&http, &endpoints(), &token, budget);
    assert_eq!(result.unwrap_err(), CloudError::Offline);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(http.pending() >= 3, "tries: {}", 5 - http.pending());
    for request in http.requests() {
        assert!(request.timeout.unwrap() <= Duration::from_secs(2));
    }

    // A spent budget sends nothing.
    let http = FakeHttpTransport::new();
    let spent = RequestBudget::until(Instant::now());
    let result = refresh_access_token(&http, &endpoints(), &token, spent);
    assert_eq!(result.unwrap_err(), CloudError::Offline);
    assert!(http.requests().is_empty());
}

#[test]
fn token_endpoint_retry_after_is_honoured() {
    let http = FakeHttpTransport::new();
    http.push_reply(ScriptedReply {
        status: 429,
        headers: vec![("Retry-After".into(), "120".into())],
        body: b"{}".to_vec(),
    });
    // 120 s fits no 10 s budget: the server's wait is known, so no blind retry.
    let budget = RequestBudget::until(Instant::now() + Duration::from_secs(10));
    let result = refresh_access_token(&http, &endpoints(), &SecretString::new("r"), budget);
    assert_eq!(
        result.unwrap_err(),
        CloudError::RateLimited {
            retry_after: Some(Duration::from_secs(120))
        }
    );
    assert_eq!(http.requests().len(), 1);
}

#[test]
fn absurd_expires_in_is_clamped_without_panic() {
    let number = |json: &str| serde_json::from_str::<serde_json::Number>(json).unwrap();
    assert_eq!(token_lifetime_secs(None), DEFAULT_EXPIRES_IN_SECS);
    assert_eq!(token_lifetime_secs(Some(&number("3599"))), 3599);
    for absurd in ["18446744073709551615", "-5", "1e300", "90000"] {
        assert_eq!(
            token_lifetime_secs(Some(&number(absurd))),
            MAX_EXPIRES_IN_SECS,
            "{absurd}"
        );
    }
    let http = FakeHttpTransport::new();
    http.push_json(
        200,
        r#"{"access_token":"a2","expires_in":18446744073709551615}"#,
    );
    let grant = refresh_access_token(
        &http,
        &endpoints(),
        &SecretString::new("1//r"),
        RequestBudget::BACKGROUND,
    )
    .unwrap();
    let left = grant.access_token.expires_at - Instant::now();
    assert!(left <= Duration::from_secs(MAX_EXPIRES_IN_SECS));
}
