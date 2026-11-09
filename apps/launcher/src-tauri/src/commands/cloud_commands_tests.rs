//! Cloud commands without Tauri: status / "signed in" rules (r2 F7), request skips
//! (signed out, no content id), the Local folder choice, sign-in refusals for the
//! providers not built in yet, base cleanup on missing-game removal (r2 F1).

use std::sync::Arc;

use cloud_sync::providers::fake_cloud_provider::FakeCloudProvider;
use cloud_sync::{RemoteManifest, SyncOptions};
use config_layers::{global_config_path, set_layer_values, CloudConfig, CloudProviderId, Value};

use crate::cloud_app_host::request_for;
use crate::cloud_game_result::SyncGameResult;
use crate::cloud_provider_factory::{current_account, current_account_with, AccountState};
use crate::cloud_sync_state::{CloudStatus, CloudSyncState};
use crate::cloud_test_harness::{fake_account, TestHost, SHA1_A, SYSTEM};
use crate::cloud_unit_guard::remove_unit_bases;
use crate::commands::cloud_commands::{status_view, validate_local_folder};
use crate::commands::cloud_sign_in_commands::sign_in_refusal;

fn config(provider: CloudProviderId, folder: &str) -> CloudConfig {
    CloudConfig {
        provider,
        local_folder: folder.to_string(),
        ..CloudConfig::default()
    }
}

fn view(cloud: &CloudSyncState, config: &CloudConfig) -> (bool, bool) {
    let data = tempfile::tempdir().unwrap();
    let account = current_account(data.path(), cloud, config);
    let view = status_view(config, &account, &CloudStatus::default(), false);
    (view.signed_in, view.needs_sign_in)
}

#[test]
fn local_folder_counts_as_signed_in_when_configured_and_reachable() {
    let cloud = CloudSyncState::default();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().to_string_lossy().into_owned();
    assert_eq!(
        view(&cloud, &config(CloudProviderId::Off, "")),
        (false, false)
    );
    let reachable = config(CloudProviderId::LocalFolder, &path);
    assert_eq!(view(&cloud, &reachable), (true, false));
    let gone = folder
        .path()
        .join("unplugged")
        .to_string_lossy()
        .into_owned();
    let missing = config(CloudProviderId::LocalFolder, &gone);
    let data = tempfile::tempdir().unwrap();
    assert!(matches!(
        current_account(data.path(), &cloud, &missing),
        AccountState::NeedsSignIn(_)
    ));
    assert_eq!(view(&cloud, &missing), (false, true));
    assert_eq!(
        view(&cloud, &config(CloudProviderId::LocalFolder, "")),
        (false, true)
    );
    let drive = config(CloudProviderId::Onedrive, "");
    let account = current_account(data.path(), &cloud, &drive);
    let shown = status_view(&drive, &account, &CloudStatus::default(), false);
    assert!(!shown.provider_available && !shown.signed_in && shown.needs_sign_in);
    let google = config(CloudProviderId::GoogleDrive, "");
    let account = current_account_with(data.path(), &cloud, &google, || {
        Some(cloud_sync::ClientCredentials {
            client_id: "cid".into(),
            client_secret: None,
        })
    });
    let shown = status_view(&google, &account, &CloudStatus::default(), false);
    assert!(shown.provider_available && !shown.signed_in && shown.needs_sign_in);
}

#[test]
fn signed_out_or_no_content_id_is_skipped() {
    let pc = TestHost::new("PC-A");
    let game = pc.game(SYSTEM, SHA1_A);
    let skipped = |reason: &str| SyncGameResult::Skipped {
        reason: reason.to_string(),
    };
    let request = request_for(pc.root(), pc.cloud(), 1, &game, SyncOptions::pull_only());
    assert_eq!(request.unwrap_err(), skipped("signed out"));
    let folder = tempfile::tempdir().unwrap();
    let entries = [
        ("cloud.provider", Some(Value::from("local_folder"))),
        (
            "cloud.local_folder",
            Some(Value::from(folder.path().to_string_lossy().into_owned())),
        ),
    ];
    set_layer_values(&global_config_path(pc.root()), &entries).unwrap();
    let ready = request_for(pc.root(), pc.cloud(), 1, &game, SyncOptions::pull_only());
    assert_eq!(ready.unwrap().units.len(), 1);
    let no_id = pc.game(SYSTEM, "");
    let request = request_for(pc.root(), pc.cloud(), 1, &no_id, SyncOptions::pull_only());
    assert_eq!(request.unwrap_err(), skipped("no content id"));
}

#[test]
fn local_folder_must_exist_and_lie_outside_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    let inside = data.path().join("cloud-here");
    std::fs::create_dir_all(&inside).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let text = |path: &std::path::Path| path.to_string_lossy().into_owned();
    assert_eq!(
        validate_local_folder(data.path(), &text(&inside))
            .unwrap_err()
            .kind,
        "invalid"
    );
    let missing = outside.path().join("missing");
    assert_eq!(
        validate_local_folder(data.path(), &text(&missing))
            .unwrap_err()
            .kind,
        "invalid"
    );
    assert_eq!(
        validate_local_folder(data.path(), "relative")
            .unwrap_err()
            .kind,
        "invalid"
    );
    let chosen = validate_local_folder(data.path(), &text(outside.path())).unwrap();
    assert_eq!(chosen, outside.path());
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "probe removed"
    );
}

#[test]
fn onedrive_dropbox_and_google_without_client_id_are_unavailable() {
    let data = tempfile::tempdir().unwrap();
    for id in [CloudProviderId::Onedrive, CloudProviderId::Dropbox] {
        assert_eq!(sign_in_refusal(id).kind, "cloud_unavailable");
        let account = current_account(data.path(), &CloudSyncState::default(), &config(id, ""));
        assert!(matches!(account, AccountState::Unavailable(found) if found == id));
    }
    let google = config(CloudProviderId::GoogleDrive, "");
    let account = current_account_with(data.path(), &CloudSyncState::default(), &google, || None);
    assert!(matches!(
        account,
        AccountState::Unavailable(CloudProviderId::GoogleDrive)
    ));
    assert_eq!(
        sign_in_refusal(CloudProviderId::LocalFolder).kind,
        "invalid"
    );
}

#[test]
fn removed_game_loses_its_bases_and_is_pulled_back_on_re_add() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    a.write(&ta, "save.srm", b"v1");
    a.sync_both(&account, &ta);
    b.pre_play(&account, &tb, SyncOptions::pull_only());
    let unit = a.units(&ta)[0].clone();
    let base = cloud_sync::BaseManifestStore::new(a.root(), &account.account_key).path_for(&unit);
    assert!(base.is_file());
    // Missing-game removal on root A: save folder + every account's base go.
    std::fs::remove_dir_all(a.save_dir(&ta)).unwrap();
    remove_unit_bases(a.root(), SYSTEM, SHA1_A).unwrap();
    assert!(!base.exists());
    // Re-added later: the first sync is a pull, never a cloud delete.
    assert_eq!(a.sync_both(&account, &ta), SyncGameResult::Synced);
    assert_eq!(a.read(&ta, "save.srm").unwrap(), b"v1");
    let manifest = fake
        .get(&format!("{}manifest.json", unit.remote_prefix()))
        .unwrap();
    let manifest = RemoteManifest::parse(&manifest).unwrap();
    assert!(manifest
        .files
        .values()
        .all(|entry| entry.deleted_at.is_none()));
    assert_eq!(
        b.read(&tb, "save.srm").unwrap(),
        b"v1",
        "root B keeps its saves"
    );
}
