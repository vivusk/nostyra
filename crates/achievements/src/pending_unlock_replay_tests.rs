#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::*;
use crate::fake_server_transport::FakeTransport;
use crate::server_transport::{form_field, request_kind};

fn credentials() -> Credentials {
    Credentials {
        user: "Tester".into(),
        token: "tok123".into(),
    }
}

fn entry(kind: PendingKind, id: u32, user: &str) -> PendingEntry {
    PendingEntry {
        kind,
        id,
        hardcore: false,
        game_hash: "0123456789abcdef0123456789abcdef".into(),
        user: user.into(),
        at_unix: unix_now() - 3600,
        score: (kind == PendingKind::Leaderboard).then_some(17),
    }
}

/// Journal in a temp dir holding `entries`.
fn journal_with(entries: &[PendingEntry]) -> (tempfile::TempDir, PendingJournal) {
    let data = tempfile::tempdir().unwrap();
    let journal = PendingJournal::new(data.path());
    for e in entries {
        journal.write(e).unwrap();
    }
    (data, journal)
}

fn replay_with(
    award: (i32, &str),
    entries: &[PendingEntry],
) -> (ReplaySummary, usize, FakeTransport) {
    let (_data, journal) = journal_with(entries);
    let fake = FakeTransport::new();
    fake.respond("awardachievement", award.0, award.1);
    let summary = replay_pending_unlocks(&journal, &fake, &credentials());
    (summary, journal.list().len(), fake)
}

#[test]
fn delivered_unlock_is_removed_and_sent_with_elapsed_seconds() {
    let (summary, left, fake) = replay_with(
        (
            200,
            "{\"Success\":true,\"Score\":15,\"AchievementID\":101,\"AchievementsRemaining\":1}",
        ),
        &[entry(PendingKind::Unlock, 101, "Tester")],
    );
    assert_eq!((summary.delivered, left), (1, 0));
    let post = &fake.posts()[0];
    assert_eq!(request_kind(post), "awardachievement");
    assert_eq!(form_field(post, "a"), Some("101"));
    assert_eq!(form_field(post, "t"), Some("tok123"));
    let elapsed: u32 = form_field(post, "o").unwrap().parse().unwrap();
    assert!((3600..3700).contains(&elapsed), "o={elapsed}");
}

#[test]
fn already_unlocked_counts_as_delivered() {
    let (summary, left, _) = replay_with(
        (
            200,
            "{\"Success\":false,\"Error\":\"User already has this achievement awarded.\"}",
        ),
        &[entry(PendingKind::Unlock, 101, "Tester")],
    );
    assert_eq!((summary.delivered, summary.dropped, left), (1, 0, 0));
}

#[test]
fn retryable_status_keeps_the_entry() {
    let (summary, left, _) = replay_with((503, ""), &[entry(PendingKind::Unlock, 101, "Tester")]);
    assert_eq!((summary.kept, left), (1, 1));
}

#[test]
fn permanent_server_error_drops_the_entry() {
    let (summary, left, _) = replay_with(
        (
            200,
            "{\"Success\":false,\"Error\":\"Achievement not found\"}",
        ),
        &[entry(PendingKind::Unlock, 101, "Tester")],
    );
    assert_eq!((summary.dropped, left), (1, 0));
}

#[test]
fn offline_stops_the_pass_and_keeps_everything() {
    let entries = [
        entry(PendingKind::Unlock, 101, "Tester"),
        entry(PendingKind::Unlock, 102, "Tester"),
    ];
    let (summary, left, fake) = replay_with((-2, "offline"), &entries);
    assert_eq!((summary.kept, left), (2, 2));
    assert_eq!(fake.posts().len(), 1, "no hammering while offline");
}

#[test]
fn rejected_token_keeps_entries_and_asks_for_login() {
    let (summary, left, _) = replay_with(
        (
            403,
            "{\"Success\":false,\"Error\":\"The access token has expired.\",\"Status\":401,\"Code\":\"expired_token\"}",
        ),
        &[entry(PendingKind::Unlock, 101, "Tester")],
    );
    assert!(summary.login_required);
    assert_eq!(left, 1);
}

#[test]
fn other_users_entries_are_never_sent() {
    let (summary, left, fake) = replay_with(
        (200, "{\"Success\":true}"),
        &[entry(PendingKind::Unlock, 101, "Someone")],
    );
    assert_eq!((summary.other_user, left), (1, 1));
    assert!(fake.posts().is_empty());
}

#[test]
fn leaderboard_submit_is_replayed_with_its_score() {
    let (_data, journal) = journal_with(&[entry(PendingKind::Leaderboard, 201, "Tester")]);
    let fake = FakeTransport::new();
    let summary = replay_pending_unlocks(&journal, &fake, &credentials());
    assert_eq!(summary.delivered, 1);
    let post = &fake.posts()[0];
    assert_eq!(request_kind(post), "submitlbentry");
    assert_eq!(
        (form_field(post, "i"), form_field(post, "s")),
        (Some("201"), Some("17"))
    );
    assert!(journal.list().is_empty());
}

#[test]
fn http_500_and_html_bodies_keep_the_entry_and_stop_the_pass() {
    for (status, body) in [
        (500, "oops"),
        (520, "<html>cf</html>"),
        (408, ""),
        (200, "<html>"),
    ] {
        let entries = [
            entry(PendingKind::Unlock, 101, "Tester"),
            entry(PendingKind::Unlock, 102, "Tester"),
        ];
        let (summary, left, fake) = replay_with((status, body), &entries);
        assert_eq!((summary.dropped, summary.kept, left), (0, 2, 2), "{status}");
        assert_eq!(fake.posts().len(), 1, "{status}");
    }
}

#[test]
fn delivered_unlock_joins_the_cached_offline_session() {
    use crate::offline_response_cache::{CachedAnswer, OfflineResponseCache};
    let unlock = entry(PendingKind::Unlock, 101, "Tester");
    let (data, journal) = journal_with(std::slice::from_ref(&unlock));
    let cache = OfflineResponseCache::new(data.path());
    let session = "{\"Success\":true,\"Unlocks\":[],\"HardcoreUnlocks\":[]}";
    let hash = Some(unlock.game_hash.as_str());
    cache
        .store(CachedAnswer::Session, "tester", hash, session)
        .unwrap();
    let summary = replay_pending_unlocks(&journal, &FakeTransport::new(), &credentials());
    assert_eq!(summary.delivered, 1);
    let cached = cache.load(CachedAnswer::Session, "Tester", hash).unwrap();
    let json: serde_json::Value = serde_json::from_str(&cached).unwrap();
    let expected = serde_json::json!([{ "ID": 101, "When": unlock.at_unix }]);
    assert_eq!(json["Unlocks"], expected);
}
