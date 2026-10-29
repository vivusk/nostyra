//! Per-frame and lifecycle calls into the client, all on the session thread:
//! `do_frame` after every real-timeline frame (normal speed after run-ahead's real
//! step, each fast-forward frame; never run-ahead's hidden frames, which the core
//! rolls back), `idle` while paused or rewinding, `reset` with the core, and the
//! disc swap (hashed here with our readers, then `rc_client_begin_change_media`,
//! M15 D8). Memory regions are (re)built lazily before each `do_frame` (D6) and
//! dropped on a disc swap; set requests wait for them (rc_client validates every
//! address with a read when a set loads).

use std::path::{Path, PathBuf};

use achievements::{content_hash, CoreMemoryLookup, RegionStatus};
use libretro_host::{CoreSession, MemoryMapSnapshot};

use super::{AchievementSession, DeferredLoad};
use crate::achievement_overlay::game_placard;

/// After one real frame of `session` (no-op without a session).
pub fn after_real_frame(achievements: &mut Option<AchievementSession>, session: &CoreSession) {
    if let Some(achievements) = achievements {
        achievements.on_real_frame(session.memory_maps(), &|id| session.memory_info(id));
    }
}

impl AchievementSession {
    /// Results that arrived → memory regions (lazy build / rebuild; a deferred set
    /// request starts once they are ready) → `do_frame` → routing of what it raised.
    pub fn on_real_frame(
        &mut self,
        maps: Option<&MemoryMapSnapshot>,
        lookup: &CoreMemoryLookup<'_>,
    ) {
        self.pump();
        if self.client.is_none() {
            return;
        }
        let status = self.memory.borrow_mut().after_real_frame(maps, lookup);
        match status {
            RegionStatus::Ready => self.start_deferred(),
            RegionStatus::Unavailable if !self.memory_unavailable => self.give_up_memory(),
            _ => {}
        }
        if let Some(client) = &self.client {
            client.do_frame();
        }
        self.pump();
    }

    /// Sends the held game load / disc change (inline dispatch completes it here, so
    /// the set is active before this frame's `do_frame`).
    fn start_deferred(&mut self) {
        let (Some(client), Some(deferred)) = (&self.client, self.deferred.take()) else {
            return;
        };
        match deferred {
            DeferredLoad::Identify(request) => {
                client.begin_identify_and_load_game(request.console_id, &request.path, request.data)
            }
            DeferredLoad::ChangeMedia(hash) => client.begin_change_media(&hash),
        }
        self.pump();
    }

    /// No memory within the frame budget (Q8): told once, the set unloaded.
    fn give_up_memory(&mut self) {
        self.memory_unavailable = true;
        self.deferred = None;
        tracing::warn!("no core memory after the frame budget; achievements off");
        self.router
            .overlay
            .announce(game_placard::memory_unavailable());
        if let Some(client) = &self.client {
            client.unload_game();
        }
    }

    /// While paused (each paused redraw, ≥ 1/s) and during rewind playback: pings,
    /// scheduled retries and arrived results keep flowing without frames.
    pub fn idle(&mut self) {
        if let Some(client) = &self.client {
            client.idle();
        }
        self.pump();
    }

    /// The core was reset: achievements re-arm.
    pub fn reset(&mut self) {
        if let Some(client) = &self.client {
            client.reset();
        }
    }

    /// Disc `index` of the playlist was inserted: regions dropped (rebuilt after the
    /// next frame), then the new disc's hash goes to rc_client once they are ready.
    pub fn disc_swapped(&mut self, index: u32) {
        self.memory.borrow_mut().reset();
        if self.client.is_none() {
            return;
        }
        let hash =
            disc_path(&self.rom, index).and_then(|path| content_hash(self.console_id, &path, None));
        match hash {
            Some(hash) => self.deferred = Some(DeferredLoad::ChangeMedia(hash)),
            None => {
                tracing::warn!(disc = index + 1, "inserted disc cannot be hashed");
                // Unidentified media ends hardcore, as rc_client does for an unknown hash.
                self.drop_to_softcore("inserted disc cannot be identified");
            }
        }
    }
}

/// Image `index` of an `.m3u` playlist (the only multi-disc launch form).
fn disc_path(rom: &Path, index: u32) -> Option<PathBuf> {
    let is_playlist = rom
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(disc_image::M3U_EXT));
    if !is_playlist {
        return None;
    }
    disc_image::playlist_entries(rom)
        .ok()?
        .into_iter()
        .nth(index as usize)
}
