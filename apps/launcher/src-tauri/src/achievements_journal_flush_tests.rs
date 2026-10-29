//! Startup journal flush with the fake RA server: skip rules, a delivered entry is
//! removed, other users' entries stay, a rejected token is reported.

use achievements::{
    save_credentials, Credentials, FakeTransport, PendingEntry, PendingJournal, PendingKind,
};

use super::*;

fn login(data: &Path) {
    let credentials = Credentials {
        user: "Tester".into(),
        token: "tok123".into(),
    };
    save_credentials(data, &credentials).unwrap();
}

fn pending(data: &Path, user: &str) {
    let entry = PendingEntry {
        kind: PendingKind::Unlock,
        id: 101,
        hardcore: false,
        game_hash: "0123456789abcdef0123456789abcdef".into(),
        user: user.into(),
        at_unix: 1_700_000_000,
        score: None,
    };
    PendingJournal::new(data).write(&entry).unwrap();
}

#[test]
fn skips_without_login_entries_or_while_a_game_runs() {
    let data = tempfile::tempdir().unwrap();
    let fake = FakeTransport::new();
    pending(data.path(), "Tester");
    assert_eq!(
        flush_journal(data.path(), &fake, false),
        FlushResult::Skipped("logged out")
    );
    login(data.path());
    assert_eq!(
        flush_journal(data.path(), &fake, true),
        FlushResult::Skipped("a game is running")
    );
    let empty = tempfile::tempdir().unwrap();
    login(empty.path());
    assert_eq!(
        flush_journal(empty.path(), &fake, false),
        FlushResult::Skipped("nothing pending")
    );
    assert!(fake.posts().is_empty(), "no request when skipped");
    assert_eq!(PendingJournal::new(data.path()).list().len(), 1);
}

#[test]
fn delivers_own_entries_and_keeps_other_users() {
    let data = tempfile::tempdir().unwrap();
    login(data.path());
    pending(data.path(), "Tester");
    pending(data.path(), "SomeoneElse");
    let fake = FakeTransport::new();
    let FlushResult::Replayed(summary) = flush_journal(data.path(), &fake, false) else {
        panic!("expected a replay");
    };
    assert_eq!((summary.delivered, summary.other_user), (1, 1));
    assert!(!summary.login_required);
    let left = PendingJournal::new(data.path()).list();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].1.user, "SomeoneElse");
}

#[test]
fn rejected_token_is_reported_and_the_entry_kept() {
    let data = tempfile::tempdir().unwrap();
    login(data.path());
    pending(data.path(), "Tester");
    let fake = FakeTransport::new();
    fake.respond(
        "awardachievement",
        401,
        "{\"Success\":false,\"Error\":\"Invalid token\",\"Code\":\"invalid_credentials\"}",
    );
    let FlushResult::Replayed(summary) = flush_journal(data.path(), &fake, false) else {
        panic!("expected a replay");
    };
    assert!(summary.login_required);
    assert_eq!(PendingJournal::new(data.path()).list().len(), 1);
}
