//! Achievement progress inside save states (sidecar `achievements_progress`, hex of
//! `rc_client_serialize_progress`) and rewind captures. Restoring a state while the
//! game set is still loading (launcher Continue / `--load-state` in the first
//! frames, before the load even starts after frame 1) keeps the blob as pending
//! progress, applied when the load completes; rcheevos refuses both calls with −29
//! meanwhile (spike).

use achievements::LoadGameState;

use super::AchievementSession;

/// Hex progress of `achievements` for a state being saved; `None` without a session
/// or a loaded game.
pub fn progress_of(achievements: &Option<AchievementSession>) -> Option<String> {
    achievements
        .as_ref()?
        .rewind_progress()
        .map(|blob| to_hex(&blob))
}

impl AchievementSession {
    /// A state loaded with `progress` (`None` = saved without one → reset).
    pub fn apply_loaded_progress(&mut self, progress: Option<&str>) {
        let Some(client) = &self.client else {
            return;
        };
        let blob = progress.and_then(|hex| {
            let blob = from_hex(hex);
            if blob.is_none() {
                tracing::warn!("state has malformed achievement progress; reset instead");
            }
            blob
        });
        if self.deferred.is_some() {
            self.router.pending_progress = Some(blob); // load starts after frame 1
            return;
        }
        match client.load_game_state() {
            LoadGameState::Done if client.is_game_loaded() => {
                if let Err(rc) = client.deserialize_progress(blob.as_deref()) {
                    tracing::warn!(rc, "achievement progress not restored");
                }
            }
            LoadGameState::AwaitLogin
            | LoadGameState::IdentifyingGame
            | LoadGameState::FetchingGameData
            | LoadGameState::StartingSession => self.router.pending_progress = Some(blob),
            _ => {} // no game set (unknown game, failed load): nothing to restore
        }
    }

    /// Raw progress blob of the loaded game (rewind captures, state saves).
    pub fn rewind_progress(&self) -> Option<Vec<u8>> {
        let client = self.client.as_ref()?;
        client
            .is_game_loaded()
            .then(|| client.serialize_progress().ok())
            .flatten()
    }

    /// Rewind playback restored a capture taken with `blob` (`None` → reset).
    pub fn restore_rewind_progress(&self, blob: Option<&[u8]>) {
        if let Some(client) = self.client.as_ref().filter(|c| c.is_game_loaded()) {
            if let Err(rc) = client.deserialize_progress(blob) {
                tracing::debug!(rc, "rewind progress not restored");
            }
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_rejects() {
        let bytes = [0u8, 1, 0xab, 0xff];
        assert_eq!(to_hex(&bytes), "0001abff");
        assert_eq!(from_hex("0001abff").as_deref(), Some(&bytes[..]));
        assert_eq!(from_hex("0001ABFF").as_deref(), Some(&bytes[..]));
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
        assert_eq!(from_hex("é1"), None);
        assert_eq!(from_hex("").as_deref(), Some(&[][..]));
    }
}
