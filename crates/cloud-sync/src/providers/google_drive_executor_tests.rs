//! The phase-02 executor scenarios over the real Google provider code and an
//! in-memory Drive: two PCs (each its own provider + folder cache, one Drive),
//! round trip incl. nested folders and a resumable-sized blob, conflicts with
//! backups, deletions and blob GC (`delete_dir` on Drive folders).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::google_drive_account::google_drive_endpoints;
use super::google_drive_fake_server::FakeDriveServer;
use super::google_drive_provider::GoogleDriveProvider;
use crate::conflict_choice::Choice;
use crate::sync_outcome::{SyncOptions, SyncStatus};
use crate::sync_test_harness::Pc;
use crate::three_way_sync_planner::ConflictKind;
use crate::{
    read_index, AccessToken, ClientCredentials, CloudStorageProvider, OAuthSession, SecretString,
    LARGE_WRITE_BYTES,
};

fn drive(server: &Arc<FakeDriveServer>) -> GoogleDriveProvider {
    let endpoints = google_drive_endpoints(ClientCredentials {
        client_id: "cid".into(),
        client_secret: None,
    });
    let access = AccessToken {
        value: SecretString::new("token"),
        expires_at: Instant::now() + Duration::from_secs(3600),
    };
    let session = OAuthSession::new(
        endpoints,
        server.clone(),
        SecretString::new("refresh"),
        Some(access),
        Box::new(|_| Ok(())),
    );
    GoogleDriveProvider::new(session, "me@example.com")
}

#[test]
fn round_trip_with_nested_folders_and_a_chunked_blob() {
    let server = Arc::new(FakeDriveServer::default());
    let (cloud_a, cloud_b) = (drive(&server), drive(&server));
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("Game [6B47BB75]"), b.game("Renamed [6B47BB75]"));
    let big: Vec<u8> = (0..LARGE_WRITE_BYTES * 2 + 77)
        .map(|i| (i % 253) as u8)
        .collect();
    a.write(&ua, "save.srm", b"sram v1");
    a.write(&ua, "PSP/SAVEDATA/X/DATA.BIN", b"nested");
    a.write(&ua, "slot1.state", &big);
    assert_eq!(a.sync_ok(&cloud_a, &ua).committed_generation, Some(1));

    let pulled = b.sync_ok(&cloud_b, &ub);
    assert_eq!(pulled.pulled.len(), 3);
    assert_eq!(b.read(&ub, "PSP/SAVEDATA/X/DATA.BIN").unwrap(), b"nested");
    assert_eq!(b.read(&ub, "slot1.state").unwrap(), big);

    b.write(&ub, "save.srm", b"sram v2 from B");
    assert_eq!(b.sync_ok(&cloud_b, &ub).committed_generation, Some(2));
    let back = a.sync_ok(&cloud_a, &ua);
    assert_eq!(back.pulled, vec!["save.srm".to_string()]);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"sram v2 from B");
    assert_eq!(a.backup_of(&ua, "local", "save.srm").unwrap(), b"sram v1");
    let nothing = a.sync_ok(&cloud_a, &ua);
    assert!(nothing.pulled.is_empty() && nothing.pushed.is_empty());
    assert_eq!(
        read_index(&cloud_b).unwrap().0.units[&ua.key()].generation,
        2
    );
}

#[test]
fn both_sides_changed_is_a_conflict_with_the_loser_backed_up() {
    let server = Arc::new(FakeDriveServer::default());
    let (cloud_a, cloud_b) = (drive(&server), drive(&server));
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"base");
    a.sync_ok(&cloud_a, &ua);
    b.sync_ok(&cloud_b, &ub);
    a.write(&ua, "save.srm", b"from A");
    b.write(&ub, "save.srm", b"from B");
    a.sync_ok(&cloud_a, &ua);

    let outcome = b.sync(&cloud_b, &ub, SyncOptions::both()).unwrap();
    assert_eq!(outcome.status, SyncStatus::NeedsChoice);
    assert_eq!(outcome.conflicts[0].kind, ConflictKind::BothChanged);
    b.sync_choosing(&cloud_b, &ub, Choice::UseRemote);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"from A");
    assert_eq!(b.backup_of(&ub, "local", "save.srm").unwrap(), b"from B");
}

#[test]
fn deletes_propagate_and_old_blob_dirs_are_collected() {
    let server = Arc::new(FakeDriveServer::default());
    let (cloud_a, cloud_b) = (drive(&server), drive(&server));
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "rtc.bin", b"r");
    for version in [b"v1", b"v2", b"v3"] {
        a.write(&ua, "save.srm", version);
        a.sync_ok(&cloud_a, &ua);
    }
    let blobs = format!("{}blobs", ua.remote_prefix());
    let dev8 = a.device.device8().to_string();
    let mut dirs: Vec<String> = cloud_b
        .list(&blobs)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    dirs.sort();
    let want: Vec<String> = [1, 2, 3].iter().map(|g| format!("{g}-{dev8}")).collect();
    assert_eq!(dirs, want, "gen 2 = previous manifest");
    a.write(&ua, "save.srm", b"v4");
    a.sync_ok(&cloud_a, &ua);
    let dirs: Vec<String> = cloud_b
        .list(&blobs)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(!dirs.contains(&format!("2-{dev8}")), "{dirs:?}");

    b.sync_ok(&cloud_b, &ub);
    b.remove(&ub, "rtc.bin");
    b.sync_ok(&cloud_b, &ub);
    a.sync_ok(&cloud_a, &ua);
    assert_eq!(a.read(&ua, "rtc.bin"), None);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"v4");
}
