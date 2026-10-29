//! Keeps the offline unlock journal (`<data>/achievements/pending/`) in step with
//! what rc_client actually sends (M15 QA L1): an entry is written when an award /
//! leaderboard submit is **sent** (exact hash, mode, score; retries reuse it) and
//! removed when the **server answer** says delivered, already unlocked or refused for
//! good. Retryable answers (offline, 5xx, shutdown −2) keep it for the next replay.
//! Removal also clears matching leftovers of earlier sessions; a file another
//! process already removed (launcher flush) is fine.

use std::path::{Path, PathBuf};

use achievements::{
    PendingEntry, PendingJournal, PendingKind, ReplayOutcome, RequestObservation, TrackedSubmission,
};

/// `(kind, id, game hash)`: what identifies one unlock / submit in the journal.
type EntryKey = (PendingKind, u32, String);

fn key_of(tracked: &TrackedSubmission) -> EntryKey {
    (tracked.kind, tracked.id, tracked.game_hash.clone())
}

pub(super) struct JournalBridge {
    journal: PendingJournal,
    user: String,
    /// Entries written this session and not yet answered for good.
    open: Vec<(EntryKey, PathBuf)>,
}

impl JournalBridge {
    pub(super) fn new(data_root: &Path, user: &str) -> Self {
        Self {
            journal: PendingJournal::new(data_root),
            user: user.to_owned(),
            open: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, observation: RequestObservation) {
        match observation {
            RequestObservation::Sent(tracked) => self.sent(&tracked),
            RequestObservation::Answered(tracked, outcome) => match outcome {
                ReplayOutcome::Delivered => self.settled(&tracked),
                ReplayOutcome::Dropped(reason) => {
                    tracing::warn!(id = tracked.id, kind = ?tracked.kind, %reason,
                        "achievement submission refused by the server");
                    self.settled(&tracked);
                }
                ReplayOutcome::Retry(_) | ReplayOutcome::LoginRequired => {}
            },
        }
    }

    /// First send of a key writes the entry (before the answer can be routed).
    fn sent(&mut self, tracked: &TrackedSubmission) {
        let key = key_of(tracked);
        if self.open.iter().any(|(open, _)| *open == key) {
            return; // a retry of an entry already on disk
        }
        let entry = PendingEntry {
            kind: tracked.kind,
            id: tracked.id,
            hardcore: tracked.hardcore,
            game_hash: tracked.game_hash.clone(),
            user: self.user.clone(),
            at_unix: chrono::Utc::now().timestamp(),
            score: tracked.score,
        };
        match self.journal.write(&entry) {
            Ok(path) => self.open.push((key, path)),
            Err(err) => {
                tracing::error!(%err, id = entry.id, "cannot journal achievement submission")
            }
        }
    }

    /// Delivered or refused for good: every journal file of this key and user goes.
    fn settled(&mut self, tracked: &TrackedSubmission) {
        let key = key_of(tracked);
        self.open.retain(|(open, _)| *open != key);
        for (path, entry) in self.journal.list() {
            let same = entry.kind == key.0 && entry.id == key.1 && entry.game_hash == key.2;
            if same && entry.user.eq_ignore_ascii_case(&self.user) {
                if let Err(err) = self.journal.remove(&path) {
                    tracing::warn!(%err, path = %path.display(), "cannot remove delivered achievement");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    fn tracked(id: u32) -> TrackedSubmission {
        TrackedSubmission {
            kind: PendingKind::Unlock,
            id,
            hardcore: false,
            game_hash: "abc".into(),
            score: None,
        }
    }

    #[test]
    fn sent_writes_once_delivered_removes_retry_keeps() {
        let data = tempfile::tempdir().unwrap();
        let mut bridge = JournalBridge::new(data.path(), "Tester");
        let journal = PendingJournal::new(data.path());
        bridge.observe(RequestObservation::Sent(tracked(1)));
        bridge.observe(RequestObservation::Answered(
            tracked(1),
            ReplayOutcome::Retry("offline".into()),
        ));
        bridge.observe(RequestObservation::Sent(tracked(1)));
        bridge.observe(RequestObservation::Sent(tracked(2)));
        assert_eq!(journal.list().len(), 2, "retry reuses the entry");
        bridge.observe(RequestObservation::Answered(
            tracked(1),
            ReplayOutcome::Delivered,
        ));
        let left: Vec<u32> = journal.list().iter().map(|(_, e)| e.id).collect();
        assert_eq!(left, [2]);
        bridge.observe(RequestObservation::Answered(
            tracked(2),
            ReplayOutcome::Dropped("no".into()),
        ));
        assert!(journal.list().is_empty());
    }

    #[test]
    fn delivery_clears_leftovers_of_this_user_only_and_tolerates_missing_files() {
        let data = tempfile::tempdir().unwrap();
        let journal = PendingJournal::new(data.path());
        let entry = |user: &str| PendingEntry {
            kind: PendingKind::Unlock,
            id: 1,
            hardcore: false,
            game_hash: "abc".into(),
            user: user.into(),
            at_unix: 1,
            score: None,
        };
        journal.write(&entry("tester")).unwrap();
        journal.write(&entry("Other")).unwrap();
        let mut bridge = JournalBridge::new(data.path(), "Tester");
        bridge.observe(RequestObservation::Sent(tracked(1)));
        // The launcher's flush took this session's file meanwhile.
        let (mine, _) = bridge.open[0].clone();
        assert_eq!(mine.1, 1);
        std::fs::remove_file(&bridge.open[0].1).unwrap();
        bridge.observe(RequestObservation::Answered(
            tracked(1),
            ReplayOutcome::Delivered,
        ));
        let users: Vec<String> = journal.list().into_iter().map(|(_, e)| e.user).collect();
        assert_eq!(users, ["Other"]);
    }
}
