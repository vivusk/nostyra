//! Offline unlock journal: one JSON file per unlock / leaderboard submit in
//! `<data>/achievements/pending/<unix_ms>-<rand>.json`, written when the event fires
//! and removed once the server confirmed it (or refused it for good). rc_client
//! retries only in memory, so this is what survives a crash or an offline session.

use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// What a journal entry replays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingKind {
    Unlock,
    Leaderboard,
}

/// Raw facts of one unlock / submit; the request is rebuilt at replay time
/// (`seconds_since_*` from `at_unix`), so a request-format change is one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingEntry {
    pub kind: PendingKind,
    /// Achievement or leaderboard id.
    pub id: u32,
    pub hardcore: bool,
    pub game_hash: String,
    /// RA user that earned it (entries of another user are kept, never sent).
    pub user: String,
    /// Unix seconds of the unlock / completion.
    pub at_unix: i64,
    /// Leaderboard score (`None` for unlocks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,
}

/// The journal folder of one data dir.
#[derive(Debug, Clone)]
pub struct PendingJournal {
    data_dir: PathBuf,
    dir: PathBuf,
}

/// Seconds since the Unix epoch (0 if the clock is before it).
pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl PendingJournal {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            dir: data_dir.join("achievements").join("pending"),
        }
    }

    /// The data dir the journal belongs to (the offline answer cache lives there too).
    pub fn data_root(&self) -> &Path {
        &self.data_dir
    }

    /// The `pending` folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Persists `entry` (temp + rename); returns its file.
    pub fn write(&self, entry: &PendingEntry) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        // Random suffix: two unlocks in the same millisecond (or two processes) never clash.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u32(entry.id);
        let path = self
            .dir
            .join(format!("{millis}-{:016x}.json", hasher.finish()));
        let json = serde_json::to_vec_pretty(entry).map_err(std::io::Error::other)?;
        config_layers::write_atomically(&path, &json)?;
        Ok(path)
    }

    /// Every readable entry, oldest first. Unparsable files are logged and skipped
    /// (left on disk for inspection).
    pub fn list(&self) -> Vec<(PathBuf, PendingEntry)> {
        let Ok(read_dir) = std::fs::read_dir(&self.dir) else {
            return Vec::new(); // no folder = nothing pending
        };
        let mut paths: Vec<PathBuf> = read_dir
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .filter_map(|path| {
                let parsed = std::fs::read(&path)
                    .map_err(|err| err.to_string())
                    .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| err.to_string()));
                match parsed {
                    Ok(entry) => Some((path, entry)),
                    Err(err) => {
                        tracing::warn!(path = %path.display(), %err, "skipping unreadable pending unlock");
                        None
                    }
                }
            })
            .collect()
    }

    /// Removes a delivered entry; already gone (another process replayed it) is fine.
    pub fn remove(&self, path: &Path) -> std::io::Result<()> {
        match std::fs::remove_file(path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    fn unlock(id: u32) -> PendingEntry {
        PendingEntry {
            kind: PendingKind::Unlock,
            id,
            hardcore: false,
            game_hash: "0123456789abcdef0123456789abcdef".into(),
            user: "Tester".into(),
            at_unix: 1_700_000_000,
            score: None,
        }
    }

    #[test]
    fn write_list_remove() {
        let data = tempfile::tempdir().unwrap();
        let journal = PendingJournal::new(data.path());
        assert!(journal.list().is_empty());
        let first = journal.write(&unlock(1)).unwrap();
        let second = journal.write(&unlock(2)).unwrap();
        assert_ne!(first, second);
        std::fs::write(journal.dir().join("junk.json"), b"not json").unwrap();
        let listed: Vec<u32> = journal.list().iter().map(|(_, e)| e.id).collect();
        assert_eq!(listed.len(), 2);
        assert!(listed.contains(&1) && listed.contains(&2));
        journal.remove(&first).unwrap();
        journal.remove(&first).unwrap();
        assert_eq!(journal.list().len(), 1);
        assert!(journal.dir().starts_with(data.path().join("achievements")));
    }

    #[test]
    fn leaderboard_entries_keep_their_score() {
        let entry = PendingEntry {
            kind: PendingKind::Leaderboard,
            score: Some(17),
            ..unlock(201)
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"kind\":\"leaderboard\"") && json.contains("\"score\":17"));
        assert_eq!(serde_json::from_str::<PendingEntry>(&json).unwrap(), entry);
        assert!(!serde_json::to_string(&unlock(1)).unwrap().contains("score"));
    }
}
