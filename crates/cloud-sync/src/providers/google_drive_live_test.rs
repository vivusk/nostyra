//! Live round trip on a real Google account (dev sign-in), skipped unless
//! `NOSTYRA_CLOUD_LIVE_GOOGLE=1`. Needs `NOSTYRA_GOOGLE_CLIENT_ID` / `_SECRET` and
//! `NOSTYRA_CLOUD_LIVE_GOOGLE_TOKEN_FILE` (a JSON file with `refresh_token`, e.g. the
//! phase-00 spike's). Works in a throw-away `nostyra-cloud/live-<random>` sub-root
//! that is deleted at the end. Never prints a token.
//!
//! `cargo test -p cloud-sync google_drive_live -- --ignored --nocapture`

use std::sync::Arc;

use super::google_drive_account::{fetch_google_account, google_drive_endpoints};
use super::google_drive_provider::GoogleDriveProvider;
use crate::sync_test_harness::Pc;
use crate::{
    client_credentials, CloudStorageProvider, HttpTransport, OAuthProvider, OAuthSession,
    RequestBudget, SecretString, LARGE_WRITE_BYTES,
};

fn live_provider() -> Option<GoogleDriveProvider> {
    if std::env::var("NOSTYRA_CLOUD_LIVE_GOOGLE").ok().as_deref() != Some("1") {
        eprintln!("skipped: set NOSTYRA_CLOUD_LIVE_GOOGLE=1");
        return None;
    }
    let credentials = client_credentials(OAuthProvider::GoogleDrive).expect("google client id");
    let token_file = std::env::var("NOSTYRA_CLOUD_LIVE_GOOGLE_TOKEN_FILE").expect("token file");
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(token_file).expect("read token file")).expect("json");
    let refresh = saved["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();
    let transport: Arc<dyn HttpTransport> = Arc::new(http_download_util::build_http_client(
        "Nostyra-live-test",
        false,
    ));
    let endpoints = google_drive_endpoints(credentials);
    let session = OAuthSession::new(
        endpoints,
        transport.clone(),
        SecretString::new(refresh),
        None,
        Box::new(|_| Ok(())),
    );
    let access = session
        .access_token(RequestBudget::BACKGROUND)
        .expect("refresh");
    let account = fetch_google_account(transport.as_ref(), &access).expect("about");
    assert!(!account.account_id.is_empty() && !account.account_label.is_empty());
    let mut drive = GoogleDriveProvider::new(session, account.account_label);
    let mut marker = [0u8; 4];
    getrandom::fill(&mut marker).unwrap();
    drive.root_subdir = format!(
        "nostyra-cloud/live-{}",
        crate::content_hash::to_hex(&marker)
    );
    eprintln!("live sub-root: {}", drive.root_subdir);
    Some(drive)
}

#[test]
#[ignore = "live Google Drive: NOSTYRA_CLOUD_LIVE_GOOGLE=1 + dev token"]
fn google_drive_live_two_pc_round_trip() {
    let Some(cloud) = live_provider() else {
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| round_trip(&cloud)));
    let root = cloud.dir_id("", false).expect("sub-root lookup");
    if let Some(id) = root {
        cloud
            .delete_folder(&id)
            .expect("clean up the live sub-root");
        eprintln!("live sub-root deleted");
    }
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn round_trip(cloud: &GoogleDriveProvider) {
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("Live [6B47BB75]"), b.game("Other [6B47BB75]"));
    let big: Vec<u8> = (0..LARGE_WRITE_BYTES * 3 + 5)
        .map(|i| (i % 241) as u8)
        .collect();
    a.write(&ua, "save.srm", b"live v1");
    a.write(&ua, "PSP/SAVEDATA/X/DATA.BIN", b"nested");
    a.write(&ua, "slot1.state", &big);
    assert_eq!(a.sync_ok(cloud, &ua).committed_generation, Some(1));

    let pulled = b.sync_ok(cloud, &ub);
    assert_eq!(pulled.pulled.len(), 3);
    assert_eq!(b.read(&ub, "slot1.state").unwrap(), big);
    b.write(&ub, "save.srm", b"live v2 from B");
    b.remove(&ub, "PSP/SAVEDATA/X/DATA.BIN");
    assert_eq!(b.sync_ok(cloud, &ub).committed_generation, Some(2));

    let back = a.sync_ok(cloud, &ua);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"live v2 from B");
    assert_eq!(a.read(&ua, "PSP/SAVEDATA/X/DATA.BIN"), None);
    assert!(back.pulled.contains(&"save.srm".to_string()));
    for version in [b"live v3", b"live v4"] {
        a.write(&ua, "save.srm", version);
        a.sync_ok(cloud, &ua);
    }
    let blobs = format!("{}blobs", ua.remote_prefix());
    let dirs: Vec<String> = cloud
        .list(&blobs)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    let gen2 = format!("2-{}", b.device.device8());
    assert!(!dirs.contains(&gen2), "GC removed {gen2}: {dirs:?}");
    let stale = crate::Rev("not-the-head".into());
    let manifest = format!("{}manifest.json", ua.remote_prefix());
    assert_eq!(
        cloud.write(&manifest, b"{}", Some(&stale)).unwrap_err(),
        crate::CloudError::RevMismatch,
        "pre-check refuses a stale revision"
    );
    eprintln!("live round trip ok ({})", cloud.account_label().len());
}
