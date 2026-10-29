//! Disk copy of the last successful RetroAchievements answers an offline session
//! needs (M15 follow-up): `login2`, `achievementsets` and `startsession`, kept per user
//! in `<data>/achievements/offline-cache/<user>/` (`login2.json`, `sets/<hash>.json`,
//! `sessions/<hash>.json`). [`crate::OfflineCacheTransport`] stores and serves them;
//! every delivered unlock is added to the cached session so it never pops again
//! offline. The login answer holds the API token: same protection as the credentials
//! file (owner-only on Unix), and logout / account switch remove the whole folder.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::credentials_file::restrict_to_owner;
use crate::offline_cache_file_names::{hash_file_name, user_key};
use crate::server_transport::RESPONSE_LIMIT_BYTES;

/// Which server answer a cache file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CachedAnswer {
    /// `login2` (one per user; holds the token).
    Login,
    /// `achievementsets` of one game hash.
    GameSets,
    /// `startsession` of one game hash (the unlock lists).
    Session,
}

impl CachedAnswer {
    /// The cacheable answer for request kind `kind` (`r=`), if any.
    pub(crate) fn of_kind(kind: &str) -> Option<Self> {
        match kind {
            "login2" => Some(Self::Login),
            "achievementsets" => Some(Self::GameSets),
            "startsession" => Some(Self::Session),
            _ => None,
        }
    }
}

/// The `offline-cache` folder of one data dir.
#[derive(Debug, Clone)]
pub(crate) struct OfflineResponseCache {
    root: PathBuf,
}

fn cache_root(data_dir: &Path) -> PathBuf {
    data_dir.join("achievements").join("offline-cache")
}

impl OfflineResponseCache {
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            root: cache_root(data_dir),
        }
    }

    /// Removes every user's cached answers (logout, account switch); absent is fine.
    pub(crate) fn clear(data_dir: &Path) -> std::io::Result<()> {
        match std::fs::remove_dir_all(cache_root(data_dir)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// File of `answer` for `user` (form value, may be URL-encoded) and game `hash`;
    /// `None` for a per-game answer without a usable hash.
    fn path(&self, answer: CachedAnswer, user: &str, hash: Option<&str>) -> Option<PathBuf> {
        let dir = self.root.join(user_key(user));
        let per_game = |folder: &str| Some(dir.join(folder).join(hash_file_name(hash?)?));
        match answer {
            CachedAnswer::Login => Some(dir.join("login2.json")),
            CachedAnswer::GameSets => per_game("sets"),
            CachedAnswer::Session => per_game("sessions"),
        }
    }

    /// Saves `body` as the latest `answer` (atomic; login owner-only on Unix).
    pub(crate) fn store(
        &self,
        answer: CachedAnswer,
        user: &str,
        hash: Option<&str>,
        body: &str,
    ) -> std::io::Result<()> {
        match self.path(answer, user, hash) {
            Some(path) => write_answer(&path, body.as_bytes(), answer == CachedAnswer::Login),
            None => Ok(()),
        }
    }

    /// The cached `answer`, if present and not larger than a server answer may be.
    pub(crate) fn load(
        &self,
        answer: CachedAnswer,
        user: &str,
        hash: Option<&str>,
    ) -> Option<String> {
        let file = std::fs::File::open(self.path(answer, user, hash)?).ok()?;
        let mut text = String::new();
        file.take(RESPONSE_LIMIT_BYTES + 1)
            .read_to_string(&mut text)
            .ok()?;
        (text.len() as u64 <= RESPONSE_LIMIT_BYTES).then_some(text)
    }

    /// Adds a delivered unlock (`When` = `when`, Unix seconds) to the cached session of
    /// (`user`, `hash`), so an offline start does not unlock it again. No-op without a
    /// cached session or when the id is already listed.
    pub(crate) fn record_unlock(&self, user: &str, hash: &str, id: u32, hardcore: bool, when: i64) {
        let session = CachedAnswer::Session;
        let (Some(path), Some(text)) = (
            self.path(session, user, Some(hash)),
            self.load(session, user, Some(hash)),
        ) else {
            return;
        };
        let Ok(mut json) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if add_unlock(&mut json, id, hardcore, when) {
            if let Err(err) = write_answer(&path, json.to_string().as_bytes(), false) {
                tracing::warn!(%err, id, "cannot record unlock in the offline session cache");
            }
        }
    }
}

/// Adds `{ID, When}` to a `startsession` answer: `HardcoreUnlocks` for a hardcore
/// unlock (rc_client counts those for both modes), else `Unlocks`. False when the id
/// is already in that list or the answer is not a JSON object.
pub(crate) fn add_unlock(session: &mut Value, id: u32, hardcore: bool, when: i64) -> bool {
    let field = if hardcore {
        "HardcoreUnlocks"
    } else {
        "Unlocks"
    };
    let Some(list) = session.as_object_mut().and_then(|object| {
        object
            .entry(field)
            .or_insert_with(|| json!([]))
            .as_array_mut()
    }) else {
        return false;
    };
    let listed = |entry: &Value| entry.get("ID").and_then(Value::as_u64) == Some(u64::from(id));
    if list.iter().any(listed) {
        return false;
    }
    list.push(json!({ "ID": id, "When": when }));
    true
}

fn write_answer(path: &Path, bytes: &[u8], owner_only: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    config_layers::write_atomically(path, bytes)?;
    if owner_only {
        restrict_to_owner(path)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "offline_response_cache_tests.rs"]
mod tests;
