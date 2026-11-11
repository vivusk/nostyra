//! Rate limits keyed on the client IP (`X-Real-IP` only from trusted proxies).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use axum::http::StatusCode;
use support::*;

async fn create_from(app: &TestApp, from: &str, real_ip: Option<&str>) -> StatusCode {
    let headers: Vec<(&str, &str)> = real_ip.map(|ip| ("x-real-ip", ip)).into_iter().collect();
    let body = Some(create_body(2, false));
    call(app, "POST", "/v1/rooms", body, from, &headers).await.0
}

#[tokio::test]
async fn eleventh_create_in_a_minute_is_429() {
    let app = test_app();
    for _ in 0..10 {
        assert_eq!(create_from(&app, CLIENT, None).await, StatusCode::OK);
    }
    assert_eq!(
        create_from(&app, CLIENT, None).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // Another client is not affected.
    assert_eq!(
        create_from(&app, "198.51.100.2:1", None).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn forwarded_ips_from_a_trusted_proxy_get_separate_buckets() {
    let app = test_app();
    let nginx = "127.0.0.1:40000";
    for _ in 0..10 {
        assert_eq!(
            create_from(&app, nginx, Some("203.0.113.1")).await,
            StatusCode::OK
        );
    }
    let over = create_from(&app, nginx, Some("203.0.113.1")).await;
    assert_eq!(over, StatusCode::TOO_MANY_REQUESTS);
    // Same proxy, other client: own bucket.
    assert_eq!(
        create_from(&app, nginx, Some("203.0.113.2")).await,
        StatusCode::OK
    );
    // Docker bridge gateway is trusted too.
    let bridge = "172.17.0.1:40000";
    assert_eq!(
        create_from(&app, bridge, Some("203.0.113.3")).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn header_from_an_untrusted_peer_is_ignored() {
    let app = test_app();
    let direct = "198.51.100.9:7000";
    for i in 0..10 {
        let spoof = format!("203.0.113.{}", i + 10);
        assert_eq!(
            create_from(&app, direct, Some(&spoof)).await,
            StatusCode::OK
        );
    }
    // A fresh spoofed header does not buy a new bucket.
    let over = create_from(&app, direct, Some("203.0.113.99")).await;
    assert_eq!(over, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn joins_are_limited_to_thirty_a_minute() {
    let app = test_app();
    let mut last = StatusCode::OK;
    for _ in 0..31 {
        last = join(&app, "ZZZZZZ", "P", "player").await.0;
        if last == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
        assert_eq!(last, StatusCode::NOT_FOUND);
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}
