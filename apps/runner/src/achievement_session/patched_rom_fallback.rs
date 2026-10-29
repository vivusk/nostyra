//! Patched ROM fallback (offline follow-up phase 03): a soft-patched cartridge is
//! hashed as patched (M15 Q7), so a self-made patch (e.g. a fan translation) is
//! unknown to RetroAchievements. With `[achievements] patched_rom_fallback` on, the
//! first "unknown game" answer starts one more load with the original file's hash,
//! always softcore; the player is told only when that load succeeds. A second
//! unknown answer shows the usual placard with the patched hash.
//!
//! rcheevos v12.5.0 makes the retry safe from routing: `rc_client_begin_load_game`
//! unloads the dummy "Unknown Game" itself and keeps the login, and completions are
//! drained into a list before they are routed.

use std::path::Path;

use achievements::{content_hash, AchievementClient, HashRequest};
use config_layers::AchievementsConfig;

use super::achievement_event_routing::SessionRouter;

/// Popup when the original ROM's set loaded for a patched game.
pub(super) const PATCHED_FALLBACK_NOTICE: &str = "RetroAchievements: patched ROM not recognised — using the original ROM's achievements (softcore)";

/// The original file's hash of a patched game and how far the retry got.
pub(super) struct PatchedRomFallback {
    original_hash: String,
    /// The patched hash RA did not know; `Some` once the retry was started.
    patched_hash: Option<String>,
    /// The success popup was shown (once).
    announced: bool,
}

impl PatchedRomFallback {
    /// Armed only for patched bytes with the setting on. The original file is hashed
    /// here, at session start (one read before the first frame, not mid-game).
    pub(super) fn arm(
        config: &AchievementsConfig,
        request: &HashRequest,
        rom: &Path,
    ) -> Option<Self> {
        if !config.patched_rom_fallback || request.data.is_none() {
            return None;
        }
        let Some(original_hash) = content_hash(request.console_id, rom, None) else {
            tracing::info!("original ROM not hashable: no patched ROM fallback");
            return None;
        };
        Some(Self {
            original_hash,
            patched_hash: None,
            announced: false,
        })
    }
}

impl SessionRouter {
    /// Unknown patched game: starts the original ROM's load (softcore) instead of the
    /// placard, once per session. `false` = not applicable, show the placard.
    pub(super) fn retry_with_original_rom(
        &mut self,
        client: &AchievementClient,
        hash: &str,
    ) -> bool {
        let Some(fallback) = self.patched_fallback.as_mut() else {
            return false;
        };
        if fallback.patched_hash.is_some() || fallback.original_hash.eq_ignore_ascii_case(hash) {
            return false;
        }
        fallback.patched_hash = Some(hash.to_owned());
        let original = fallback.original_hash.clone();
        tracing::info!(patched = %hash, %original, "patched ROM unknown: trying the original ROM's hash");
        if client.hardcore_enabled() {
            // The session gates follow in `pump` (`follow_client_hardcore`).
            client.set_hardcore_enabled(false);
            self.notify("Hardcore disabled: patched ROM".into());
        }
        client.begin_load_game(&original);
        true
    }

    /// The hash the unknown placard names: the patched one after a failed retry.
    pub(super) fn unknown_placard_hash(&self, hash: String) -> String {
        self.patched_fallback
            .as_ref()
            .and_then(|f| f.patched_hash.clone())
            .unwrap_or(hash)
    }

    /// A game load succeeded: the fallback popup when it was the original ROM's set.
    pub(super) fn announce_patched_fallback(&mut self) {
        let Some(fallback) = self.patched_fallback.as_mut() else {
            return;
        };
        if fallback.patched_hash.is_some() && !fallback.announced {
            fallback.announced = true;
            self.notify(PATCHED_FALLBACK_NOTICE.into());
        }
    }
}
