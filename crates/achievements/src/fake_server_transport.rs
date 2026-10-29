//! In-process RetroAchievements server for tests and the runner's debug
//! `NOSTYRA_RA_FAKE_SERVER=<dir>` mode: canned JSON keyed by the request kind (`r=`),
//! response shapes from rcheevos v12.5.0 `test/test_rc_client.c`. Never opens a socket.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex};

use crate::server_transport::{
    form_field, request_kind, ServerRequest, ServerResponse, ServerTransport,
};

/// What the fake answers for one request kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeReply {
    /// HTTP status + body.
    Respond(i32, String),
    /// Blocks the calling (worker) thread until [`FakeTransport::release`], then answers
    /// `(200, "{}")`: a hung connection for shutdown tests.
    Block,
}

/// Canned-response transport. Records every request body it receives.
#[derive(Debug, Default)]
pub struct FakeTransport {
    replies: Mutex<HashMap<String, FakeReply>>,
    posts: Mutex<Vec<String>>,
    released: Mutex<bool>,
    release_signal: Condvar,
    /// Files served by [`ServerTransport::download`] (URL → bytes) and the URLs asked.
    downloads: Mutex<HashMap<String, Vec<u8>>>,
    gets: Mutex<Vec<String>>,
    /// When set, `achievementsets` for any other hash (`m=`) answers "unknown game".
    known_hash: Mutex<Option<String>>,
}

/// Default login answer: user `Tester`, token `tok123`, 10 points.
pub const FAKE_LOGIN_JSON: &str = "{\"Success\":true,\"User\":\"Tester\",\"Token\":\"tok123\",\
    \"Score\":10,\"SoftcoreScore\":3,\"Messages\":0,\"Permissions\":1,\"AccountType\":\"Registered\"}";
/// `achievementsets` answer for a hash RA does not know.
pub const FAKE_UNKNOWN_GAME_JSON: &str = "{\"Success\":true,\"GameId\":0,\"Title\":\"\",\
    \"ConsoleId\":0,\"ImageIconUrl\":\"\",\"RichPresenceGameId\":0,\"RichPresencePatch\":\"\",\"Sets\":[]}";
const START_SESSION_JSON: &str =
    "{\"Success\":true,\"Unlocks\":[],\"HardcoreUnlocks\":[],\"ServerNow\":1700000000}";
const AWARD_JSON: &str = "{\"Success\":true,\"Score\":15,\"SoftcoreScore\":8,\
    \"AchievementID\":0,\"AchievementsRemaining\":1}";
const SUBMIT_JSON: &str = "{\"Success\":true,\"Response\":{\"Score\":17,\"BestScore\":17,\
    \"TopEntries\":[{\"User\":\"Tester\",\"Score\":17,\"Rank\":1}],\"RankInfo\":{\"Rank\":1,\"NumEntries\":\"1\"}}}";

impl FakeTransport {
    /// Fake with working login, unknown game, session, award, submit and ping answers.
    pub fn new() -> Self {
        let fake = Self::default();
        for (kind, body) in [
            ("login2", FAKE_LOGIN_JSON),
            ("achievementsets", FAKE_UNKNOWN_GAME_JSON),
            ("startsession", START_SESSION_JSON),
            ("awardachievement", AWARD_JSON),
            ("submitlbentry", SUBMIT_JSON),
            ("ping", "{\"Success\":true}"),
        ] {
            fake.set_reply(kind, FakeReply::Respond(200, body.to_owned()));
        }
        fake
    }

    /// Answers the canned game set only for `hash` (others: unknown game), so a loaded
    /// set proves the content was identified.
    pub fn answer_set_only_for(&self, hash: &str) {
        *lock(&self.known_hash) = Some(hash.to_ascii_lowercase());
    }

    /// Replaces the answer for one request kind.
    pub fn set_reply(&self, kind: &str, reply: FakeReply) {
        lock(&self.replies).insert(kind.to_owned(), reply);
    }

    /// Shorthand: answer `kind` with `(status, body)`.
    pub fn respond(&self, kind: &str, status: i32, body: &str) {
        self.set_reply(kind, FakeReply::Respond(status, body.to_owned()));
    }

    /// Every request body received so far, in order.
    pub fn posts(&self) -> Vec<String> {
        lock(&self.posts).clone()
    }

    /// Request kinds received so far, in order.
    pub fn kinds(&self) -> Vec<String> {
        self.posts()
            .iter()
            .map(|post| request_kind(post).to_owned())
            .collect()
    }

    /// Serves `bytes` for GET `url` (badge images).
    pub fn serve_download(&self, url: &str, bytes: Vec<u8>) {
        lock(&self.downloads).insert(url.to_owned(), bytes);
    }

    /// Every URL asked through [`ServerTransport::download`], in order.
    pub fn downloads_requested(&self) -> Vec<String> {
        lock(&self.gets).clone()
    }

    /// Lets every [`FakeReply::Block`]ed call (now and later) return.
    pub fn release(&self) {
        *lock(&self.released) = true;
        self.release_signal.notify_all();
    }

    fn knows_hash(&self, post_data: &str) -> bool {
        let known = lock(&self.known_hash);
        known.as_deref().is_none_or(|hash| {
            form_field(post_data, "m").is_some_and(|m| m.eq_ignore_ascii_case(hash))
        })
    }

    fn wait_for_release(&self) {
        let mut released = lock(&self.released);
        while !*released {
            released = match self.release_signal.wait(released) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

impl ServerTransport for FakeTransport {
    fn call(&self, request: &ServerRequest) -> ServerResponse {
        lock(&self.posts).push(request.post_data.clone());
        let kind = request_kind(&request.post_data);
        if kind == "achievementsets" && !self.knows_hash(&request.post_data) {
            return ServerResponse {
                status: 200,
                body: FAKE_UNKNOWN_GAME_JSON.to_owned(),
            };
        }
        let reply = lock(&self.replies).get(kind).cloned();
        match reply {
            Some(FakeReply::Respond(status, body)) => ServerResponse { status, body },
            Some(FakeReply::Block) => {
                self.wait_for_release();
                ServerResponse {
                    status: 200,
                    body: "{}".into(),
                }
            }
            None => ServerResponse {
                status: 200,
                body: "{\"Success\":false,\"Error\":\"unknown request\"}".into(),
            },
        }
    }

    fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        lock(&self.gets).push(url.to_owned());
        match lock(&self.downloads).get(url) {
            Some(bytes) if bytes.len() as u64 <= limit => Ok(bytes.clone()),
            Some(_) => Err("too large".into()),
            None => Err("not found".into()),
        }
    }
}

/// Poison-tolerant lock: a panicking test thread must not cascade into others.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
