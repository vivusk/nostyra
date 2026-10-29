//! Safe, single-thread rc_client: one per runner session. `!Send` (it owns raw
//! pointers), so every rcheevos call, callback, event and memory read happens on the
//! thread that created it; only the transport crosses to the HTTP worker.

use std::path::Path;
use std::sync::Arc;

use crate::achievement_events::{AchievementEvent, ClientCompletion};
use crate::achievement_info::*;
use crate::client_state::{ClientState, MemoryReader};
use crate::ffi::RawClient;
use crate::request_http_worker::TransportJob;
use crate::request_observer::RequestObservation;
use crate::request_queue::{RequestDispatch, RequestQueue};
use crate::server_transport::ServerTransport;

/// The client failed to start (rcheevos could not allocate).
#[derive(Debug, thiserror::Error)]
#[error("cannot create the rcheevos client")]
pub struct ClientCreateError;

/// One rcheevos `rc_client` with its request queue and event queues.
pub struct AchievementClient {
    pub(crate) raw: Option<RawClient>,
}

impl AchievementClient {
    /// Creates a softcore client. `dispatch` picks inline (tests) or worker (runner)
    /// request execution; `memory` serves rc_client's memory reads.
    pub fn new(
        transport: Arc<dyn ServerTransport>,
        dispatch: RequestDispatch,
        memory: MemoryReader,
    ) -> Result<Self, ClientCreateError> {
        let state = ClientState::new(memory, RequestQueue::new(transport, dispatch));
        let raw = RawClient::create(Box::new(state)).ok_or(ClientCreateError)?;
        Ok(Self { raw: Some(raw) })
    }

    fn raw(&self) -> &RawClient {
        // Only shutdown takes the raw client, and it consumes `self`.
        self.raw
            .as_ref()
            .unwrap_or_else(|| unreachable!("raw client taken"))
    }

    /// Hardcore on/off; read at launch only (a fresh boot is the required reset).
    pub fn set_hardcore_enabled(&self, enabled: bool) {
        self.raw().set_hardcore_enabled(enabled);
    }

    pub fn hardcore_enabled(&self) -> bool {
        self.raw().hardcore_enabled()
    }

    /// `rcheevos/<version>` for the User-Agent.
    pub fn user_agent_clause(&self) -> String {
        self.raw().user_agent_clause()
    }

    /// Starts a token login; the result arrives as a [`ClientCompletion`].
    pub fn begin_login_with_token(&self, username: &str, token: &str) {
        self.raw().begin_login_with_token(username, token);
    }

    /// Hashes `path` (or `data`, the bytes the core received) for `console_id` and
    /// loads its set. Unknown hash → `RC_NO_GAME_LOADED`, [`Self::game_info`] keeps the hash.
    pub fn begin_identify_and_load_game(
        &self,
        console_id: u32,
        path: &Path,
        data: Option<Vec<u8>>,
    ) {
        self.raw()
            .begin_identify_and_load_game(console_id, &path.to_string_lossy(), data);
    }

    /// Loads the set for an already computed hash.
    pub fn begin_load_game(&self, hash: &str) {
        self.raw().begin_load_game(hash);
    }

    /// Disc swap with a hash the caller computed (D8: rc_client ignores our readers).
    pub fn begin_change_media(&self, hash: &str) {
        self.raw().begin_change_media(hash);
    }

    pub fn load_game_state(&self) -> LoadGameState {
        LoadGameState::from_raw(self.raw().load_game_state())
    }

    pub fn is_game_loaded(&self) -> bool {
        self.raw().is_game_loaded()
    }

    pub fn unload_game(&self) {
        self.raw().unload_game();
    }

    /// Processes one real frame (memory reads, triggers, events).
    pub fn do_frame(&self) {
        self.raw().do_frame();
    }

    /// Keeps pings / scheduled retries going while paused (call ≥ 1/s).
    pub fn idle(&self) {
        self.raw().idle();
    }

    /// Core was reset: achievements re-arm.
    pub fn reset(&self) {
        self.raw().reset();
    }

    /// `(pause allowed, frames until it is)` (hardcore pause rate limit).
    pub fn can_pause(&self) -> (bool, u32) {
        self.raw().can_pause()
    }

    /// Runtime progress blob for a save state; `Err(rc)` while no game is loaded.
    pub fn serialize_progress(&self) -> Result<Vec<u8>, i32> {
        self.raw().serialize_progress()
    }

    /// Restores a blob; `None` resets to fresh (state saved without progress).
    pub fn deserialize_progress(&self, blob: Option<&[u8]>) -> Result<(), i32> {
        self.raw().deserialize_progress(blob)
    }

    /// Achievements by bucket (`category` 1 core / 2 unofficial / 3 both; `grouping` 0 lock / 1 progress).
    pub fn achievement_list(&self, category: i32, grouping: i32) -> Vec<AchievementBucket> {
        self.raw().achievement_list(category, grouping)
    }

    /// Leaderboards by bucket (`grouping` 0 none / 1 tracking).
    pub fn leaderboard_list(&self, grouping: i32) -> Vec<LeaderboardBucket> {
        self.raw().leaderboard_list(grouping)
    }

    pub fn user_info(&self) -> Option<UserInfo> {
        self.raw().user_info()
    }

    pub fn game_info(&self) -> Option<GameInfo> {
        self.raw().game_info()
    }

    pub fn user_game_summary(&self) -> GameSummary {
        self.raw().user_game_summary()
    }

    /// Fires callbacks of arrived HTTP results (worker mode; before `do_frame` / `idle`).
    pub fn complete_ready_requests(&self) -> usize {
        self.raw().state().queue.complete_ready()
    }

    /// Requests still waiting for an answer.
    pub fn pending_requests(&self) -> usize {
        self.raw().state().queue.pending_count()
    }

    /// Events raised since the last drain, in order.
    pub fn drain_events(&self) -> Vec<AchievementEvent> {
        std::mem::take(&mut *self.raw().state().events.borrow_mut())
    }

    /// Award / submit traffic since the last drain, in order (offline journal, QA L1).
    pub fn drain_request_observations(&self) -> Vec<RequestObservation> {
        self.raw().state().queue.drain_observations()
    }

    /// Runs `job` on the HTTP worker FIFO (inline dispatch: right now).
    pub fn run_transport_job(&self, job: TransportJob) {
        self.raw().state().queue.run_job(job, false);
    }

    /// Like [`Self::run_transport_job`], but only while nothing else waits (badges).
    pub fn run_background_job(&self, job: TransportJob) {
        self.raw().state().queue.run_job(job, true);
    }

    /// `begin_*` results since the last drain, in order.
    pub fn drain_completions(&self) -> Vec<ClientCompletion> {
        std::mem::take(&mut *self.raw().state().completions.borrow_mut())
    }
}

#[cfg(test)]
#[path = "request_observer_client_tests.rs"]
mod observer_tests;
#[cfg(test)]
#[path = "achievement_client_shutdown_tests.rs"]
mod shutdown_tests;
#[cfg(test)]
#[path = "achievement_client_test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "achievement_client_tests.rs"]
mod tests;
