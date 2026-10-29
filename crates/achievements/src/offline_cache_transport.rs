//! Offline achievement sessions (M15 follow-up): a [`ServerTransport`] decorator that
//! keeps the last successful `login2` / `achievementsets` / `startsession` answers on
//! disk ([`OfflineResponseCache`]) and answers those kinds from it when the server is
//! unreachable, so rc_client loads a game played online before with no change of its
//! own. Award / submit / ping / downloads always reach the real transport: they fail
//! offline (rc_client retries in memory, the journal keeps them) and succeed again as
//! soon as the network is back.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::offline_cache_file_names::url_decode;
use crate::offline_cache_state::OfflineCacheState;
use crate::offline_response_cache::{add_unlock, CachedAnswer, OfflineResponseCache};
use crate::pending_unlock_journal::{unix_now, PendingJournal, PendingKind};
use crate::server_transport::{
    form_field, request_kind, ServerRequest, ServerResponse, ServerTransport, STATUS_NO_RESPONSE,
};

/// Body of the "no response" answer for a cacheable request with nothing cached while
/// offline; rc_client shows it as the error (`RetroAchievements: offline: …`).
pub const OFFLINE_NO_CACHE_BODY: &str = "offline: play once online first";

/// The cache decorator around the real (or fake) transport.
pub struct OfflineCacheTransport {
    inner: Arc<dyn ServerTransport>,
    cache: OfflineResponseCache,
    journal: PendingJournal,
    state: Arc<OfflineCacheState>,
    /// Sticky: the server once gave no answer (−2 / gateway status), so cacheable
    /// kinds skip it from now on instead of each waiting for the connect timeout.
    offline: AtomicBool,
}

/// What picks a cacheable request's file: answer kind, user (form value), game hash.
struct CacheKey<'a> {
    answer: CachedAnswer,
    user: &'a str,
    hash: Option<&'a str>,
}

impl OfflineCacheTransport {
    /// Wraps `inner`; cache and journal of `data_dir`.
    pub fn new(inner: Arc<dyn ServerTransport>, data_dir: &Path) -> Self {
        Self {
            inner,
            cache: OfflineResponseCache::new(data_dir),
            journal: PendingJournal::new(data_dir),
            state: Arc::default(),
            offline: AtomicBool::new(false),
        }
    }

    /// The served-from-cache flags (read by the session after each pump).
    pub fn state(&self) -> Arc<OfflineCacheState> {
        self.state.clone()
    }

    /// Answer for `key` from the cache, or "no response" with [`OFFLINE_NO_CACHE_BODY`].
    fn serve(&self, key: &CacheKey<'_>, post: &str) -> ServerResponse {
        let cached = self.cache.load(key.answer, key.user, key.hash);
        let body = match (key.answer, cached) {
            (_, None) => None,
            (CachedAnswer::Login, Some(body)) => token_matches(&body, post).then_some(body),
            (CachedAnswer::GameSets, Some(body)) => Some(body),
            (CachedAnswer::Session, Some(body)) => Some(self.with_journal_unlocks(body, key)),
        };
        let Some(body) = body else {
            return ServerResponse::no_response(OFFLINE_NO_CACHE_BODY);
        };
        let known_game = match key.answer {
            CachedAnswer::Login => false,
            CachedAnswer::GameSets => json_of(&body)
                .and_then(|json| json.get("GameId")?.as_u64())
                .is_some_and(|id| id > 0),
            CachedAnswer::Session => true,
        };
        self.state.mark_served(known_game);
        tracing::info!(answer = ?key.answer, "achievement answer served from the offline cache");
        ServerResponse { status: 200, body }
    }

    /// A cached session plus the journal's not yet delivered unlocks of this user and
    /// game (`When` = unlock time), so offline unlocks of earlier runs do not pop again.
    fn with_journal_unlocks(&self, body: String, key: &CacheKey<'_>) -> String {
        let (Some(mut json), Some(hash)) = (json_of(&body), key.hash) else {
            return body;
        };
        let user = url_decode(key.user);
        let pending = self.journal.list().into_iter().map(|(_, entry)| entry);
        for entry in pending.filter(|entry| {
            entry.kind == PendingKind::Unlock
                && entry.user.eq_ignore_ascii_case(&user)
                && entry.game_hash.eq_ignore_ascii_case(hash)
        }) {
            add_unlock(&mut json, entry.id, entry.hardcore, entry.at_unix);
        }
        json.to_string()
    }

    /// After a request that went to the server: remember "offline"; a delivered award
    /// joins the cached session.
    fn after_server_answer(&self, post: &str, response: &ServerResponse) {
        if is_offline_answer(response) {
            self.offline.store(true, Ordering::Relaxed);
        } else if request_kind(post) == "awardachievement" && is_success(response) {
            let field = |key| form_field(post, key);
            let id = field("a").and_then(|id| id.parse().ok());
            if let (Some(user), Some(hash), Some(id)) = (field("u"), field("m"), id) {
                let hardcore = field("h") == Some("1");
                self.cache
                    .record_unlock(user, hash, id, hardcore, unix_now());
            }
        }
    }
}

impl ServerTransport for OfflineCacheTransport {
    fn call(&self, request: &ServerRequest) -> ServerResponse {
        let post = request.post_data.as_str();
        let Some(key) = cache_key(post) else {
            let response = self.inner.call(request);
            self.after_server_answer(post, &response);
            return response;
        };
        if !self.offline.load(Ordering::Relaxed) {
            let response = self.inner.call(request);
            self.after_server_answer(post, &response);
            if !is_offline_answer(&response) {
                if is_success(&response) {
                    if let Err(err) =
                        self.cache
                            .store(key.answer, key.user, key.hash, &response.body)
                    {
                        tracing::warn!(%err, "cannot write the offline achievement cache");
                    }
                }
                return response;
            }
            tracing::info!("achievement server unreachable: using the offline cache");
        }
        self.serve(&key, post)
    }

    fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.inner.download(url, limit)
    }
}

/// Cacheable request with its user; per-game kinds need `m=` (the `g=` path is not cached).
fn cache_key(post: &str) -> Option<CacheKey<'_>> {
    let answer = CachedAnswer::of_kind(request_kind(post))?;
    let hash = form_field(post, "m");
    if answer != CachedAnswer::Login && hash.is_none() {
        return None;
    }
    let user = form_field(post, "u")?;
    Some(CacheKey { answer, user, hash })
}

/// No server answer: −2, or a gateway status of a server that is down.
fn is_offline_answer(response: &ServerResponse) -> bool {
    matches!(
        response.status,
        STATUS_NO_RESPONSE | 502 | 503 | 504 | 521..=525
    )
}

/// HTTP 200 with `"Success": true` (error answers are never cached).
fn is_success(response: &ServerResponse) -> bool {
    response.status == 200
        && json_of(&response.body).and_then(|json| json.get("Success")?.as_bool()) == Some(true)
}

fn json_of(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

/// A cached login is served only for the token it was answered with (`t=`).
fn token_matches(body: &str, post: &str) -> bool {
    let cached = json_of(body).and_then(|json| json.get("Token")?.as_str().map(str::to_owned));
    let sent = form_field(post, "t").map(url_decode);
    cached.is_some_and(|token| !token.is_empty() && Some(token) == sent)
}

#[cfg(test)]
#[path = "offline_cache_transport_tests.rs"]
mod tests;
