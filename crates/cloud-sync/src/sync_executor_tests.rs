//! Two data roots + the Local folder provider: round trips, conflicts, backups,
//! deletions. Faults, races and pull-only runs are in the sibling test modules.

use crate::conflict_choice::Choice;
use crate::sync_outcome::{SyncOptions, SyncStatus};
use crate::sync_test_harness::{local_folder, Pc};
use crate::three_way_sync_planner::ConflictKind;
use crate::{changed_units, read_index, CloudStorageProvider, SyncScope, BACKUPS_KEPT_PER_UNIT};

#[test]
fn sram_round_trip_across_renamed_rom_folders() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("Game [6B47BB75]"), b.game("Other Name [6B47BB75]"));
    a.write(&ua, "save.srm", b"sram v1");
    a.write(&ua, "PSP/SAVEDATA/X/DATA.BIN", b"nested");
    let pushed = a.sync_ok(&cloud, &ua);
    assert_eq!(pushed.pushed.len(), 2);
    assert_eq!(pushed.committed_generation, Some(1));

    let pulled = b.sync_ok(&cloud, &ub);
    assert_eq!(pulled.pulled.len(), 2);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"sram v1");
    assert_eq!(b.read(&ub, "PSP/SAVEDATA/X/DATA.BIN").unwrap(), b"nested");
    assert!(b.staging_empty(&ub));

    b.write(&ub, "save.srm", b"sram v2 from B");
    assert_eq!(b.sync_ok(&cloud, &ub).committed_generation, Some(2));
    let back = a.sync_ok(&cloud, &ua);
    assert_eq!(back.pulled, vec!["save.srm".to_string()]);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"sram v2 from B");
    assert_eq!(a.backup_of(&ua, "local", "save.srm").unwrap(), b"sram v1");

    let nothing = a.sync_ok(&cloud, &ua);
    assert!(nothing.pulled.is_empty() && nothing.pushed.is_empty());
    assert_eq!(nothing.committed_generation, None);

    let (index, _) = read_index(&cloud).unwrap();
    assert_eq!(index.units[&ua.key()].generation, 2);
    let mut bases = std::collections::BTreeMap::new();
    assert_eq!(changed_units(&index, &bases), vec![ua.key()]);
    bases.insert(ua.key(), a.base(&ua).unwrap().manifest.generation);
    assert!(changed_units(&index, &bases).is_empty());
}

#[test]
fn states_toggle_off_on_one_pc_deletes_nothing_elsewhere() {
    let (_dir, cloud) = local_folder();
    let (a, mut b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"s");
    a.write(&ua, "slot1.state", b"state");
    a.sync_ok(&cloud, &ua);
    b.scope = SyncScope {
        sync_states: false,
        ..SyncScope::default()
    };
    let pulled = b.sync_ok(&cloud, &ub);
    assert_eq!(pulled.pulled, vec!["save.srm".to_string()]);
    assert_eq!(b.read(&ub, "slot1.state"), None);
    b.write(&ub, "save.srm", b"s2");
    b.write(&ub, "slot2.state", b"local only");
    b.sync_ok(&cloud, &ub);
    let back = a.sync_ok(&cloud, &ua);
    assert!(back.deleted_local.is_empty());
    assert_eq!(a.read(&ua, "slot1.state").unwrap(), b"state");
    assert_eq!(
        a.read(&ua, "slot2.state"),
        None,
        "B never pushed its states"
    );
}

#[test]
fn ps2_memcards_are_one_shared_unit() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ma, mb) = (a.memcards(), b.memcards());
    a.write(&ma, "Mcd001.ps2", b"card");
    a.sync_ok(&cloud, &ma);
    b.sync_ok(&cloud, &mb);
    assert_eq!(b.read(&mb, "Mcd001.ps2").unwrap(), b"card");
    assert!(cloud.read("shared/ps2-memcards/manifest.json").is_ok());
}

#[test]
fn conflict_is_returned_before_any_write_then_both_choices_work() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"base");
    a.sync_ok(&cloud, &ua);
    b.sync_ok(&cloud, &ub);
    a.write(&ua, "save.srm", b"A wins?");
    a.sync_ok(&cloud, &ua);
    b.write(&ub, "save.srm", b"B wins?");
    let before = std::fs::read(cloud.root().join(ub.remote_prefix()).join("manifest.json"));
    let base_before = b.base(&ub);

    let asked = b.sync(&cloud, &ub, SyncOptions::both()).unwrap();
    assert_eq!(asked.status, SyncStatus::NeedsChoice);
    assert_eq!(asked.conflicts[0].kind, ConflictKind::BothChanged);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"B wins?");
    assert_eq!(b.base(&ub), base_before, "no base write");
    let after = std::fs::read(cloud.root().join(ub.remote_prefix()).join("manifest.json"));
    assert_eq!(before.unwrap(), after.unwrap(), "no remote write");

    let kept = b.sync_choosing(&cloud, &ub, Choice::KeepLocal);
    assert_eq!(kept.pushed, vec!["save.srm".to_string()]);
    assert_eq!(b.backup_of(&ub, "cloud", "save.srm").unwrap(), b"A wins?");
    a.sync_ok(&cloud, &ua);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"B wins?");

    a.write(&ua, "save.srm", b"A again");
    a.sync_ok(&cloud, &ua);
    b.write(&ub, "save.srm", b"B again");
    let used = b.sync_choosing(&cloud, &ub, Choice::UseRemote);
    assert_eq!(used.pulled, vec!["save.srm".to_string()]);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"A again");
    assert_eq!(b.backup_of(&ub, "local", "save.srm").unwrap(), b"B again");
}

#[test]
fn deletions_propagate_with_a_backup_and_delete_vs_modify_conflicts() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    for rel in ["save.srm", "slot1.state", "slot2.state"] {
        a.write(&ua, rel, rel.as_bytes());
    }
    a.sync_ok(&cloud, &ua);
    b.sync_ok(&cloud, &ub);
    a.remove(&ua, "slot1.state");
    assert_eq!(
        a.sync_ok(&cloud, &ua).deleted_remote,
        vec!["slot1.state".to_string()]
    );
    let gone = b.sync_ok(&cloud, &ub);
    assert_eq!(gone.deleted_local, vec!["slot1.state".to_string()]);
    assert_eq!(b.read(&ub, "slot1.state"), None);
    assert_eq!(
        b.backup_of(&ub, "local", "slot1.state").unwrap(),
        b"slot1.state"
    );

    a.remove(&ua, "slot2.state");
    a.sync_ok(&cloud, &ua);
    b.write(&ub, "slot2.state", b"B changed it");
    let asked = b.sync(&cloud, &ub, SyncOptions::both()).unwrap();
    assert_eq!(
        asked.conflicts[0].kind,
        ConflictKind::LocalChangedRemoteDeleted
    );
    b.sync_choosing(&cloud, &ub, Choice::KeepLocal);
    a.sync_ok(&cloud, &ua);
    assert_eq!(a.read(&ua, "slot2.state").unwrap(), b"B changed it");
}

#[test]
fn only_the_newest_backups_are_kept() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"v0");
    a.sync_ok(&cloud, &ua);
    b.sync_ok(&cloud, &ub);
    for round in 1..=BACKUPS_KEPT_PER_UNIT + 1 {
        a.write(&ua, "save.srm", format!("v{round}").as_bytes());
        a.sync_ok(&cloud, &ua);
        b.sync_ok(&cloud, &ub);
    }
    let dirs = b.backup_dirs(&ub);
    assert_eq!(dirs.len(), BACKUPS_KEPT_PER_UNIT, "{dirs:?}");
    let newest = format!("v{}", BACKUPS_KEPT_PER_UNIT);
    assert_eq!(
        b.backup_of(&ub, "local", "save.srm").unwrap(),
        newest.as_bytes()
    );
}
