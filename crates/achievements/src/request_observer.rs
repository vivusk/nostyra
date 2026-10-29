//! What the session needs to know about unlock / leaderboard-submit traffic to keep
//! the offline journal exact (M15 QA L1): when rc_client **sends** an award or a
//! submit (journal entry written, with the exact hash, mode and score) and how the
//! **server answered** it (entry removed when delivered, "already unlocked" or refused
//! for good). Recorded by the request queue on the client thread, in order, and
//! drained by the session after `do_frame` / `idle`.

use std::cell::RefCell;

use crate::pending_unlock_journal::PendingKind;
use crate::pending_unlock_replay::{classify, ReplayOutcome};
use crate::server_transport::{form_field, request_kind, ServerRequest, ServerResponse};

/// The journal-relevant facts of one award / submit request (no token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedSubmission {
    pub kind: PendingKind,
    /// Achievement (`a=`) or leaderboard (`i=`) id.
    pub id: u32,
    /// `h=1` for awards; submits are hardcore-only in rcheevos v12.5.0 (M15 D3).
    pub hardcore: bool,
    /// Game hash (`m=`).
    pub game_hash: String,
    /// Leaderboard score (`s=`).
    pub score: Option<i32>,
}

/// One observation, in the order the queue saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestObservation {
    /// rc_client sent the request (first try or a retry).
    Sent(TrackedSubmission),
    /// The answer arrived (or −2 at shutdown), classified like a journal replay.
    Answered(TrackedSubmission, ReplayOutcome),
}

/// Award / submit facts of `request`; `None` for every other request kind.
pub(crate) fn track(request: &ServerRequest) -> Option<TrackedSubmission> {
    let post = request.post_data.as_str();
    let field = |key| form_field(post, key);
    let (kind, id_key) = match request_kind(post) {
        "awardachievement" => (PendingKind::Unlock, "a"),
        "submitlbentry" => (PendingKind::Leaderboard, "i"),
        _ => return None,
    };
    let score = match kind {
        PendingKind::Unlock => None,
        PendingKind::Leaderboard => Some(field("s")?.parse().ok()?),
    };
    Some(TrackedSubmission {
        kind,
        id: field(id_key)?.parse().ok()?,
        hardcore: kind == PendingKind::Leaderboard || field("h") == Some("1"),
        game_hash: field("m")?.to_owned(),
        score,
    })
}

/// Ordered observation log of one request queue (client thread only).
#[derive(Default)]
pub(crate) struct RequestObserver {
    log: RefCell<Vec<RequestObservation>>,
}

impl RequestObserver {
    pub(crate) fn sent(&self, tracked: &TrackedSubmission) {
        self.log
            .borrow_mut()
            .push(RequestObservation::Sent(tracked.clone()));
    }

    pub(crate) fn answered(&self, tracked: TrackedSubmission, response: &ServerResponse) {
        let outcome = classify(tracked.kind, response);
        self.log
            .borrow_mut()
            .push(RequestObservation::Answered(tracked, outcome));
    }

    pub(crate) fn drain(&self) -> Vec<RequestObservation> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    fn request(post: &str) -> ServerRequest {
        ServerRequest {
            url: "https://x/dorequest.php".into(),
            post_data: post.into(),
            content_type: String::new(),
        }
    }

    #[test]
    fn award_and_submit_are_tracked_others_not() {
        let award = track(&request("r=awardachievement&u=T&t=x&a=101&h=0&m=abc&v=1")).unwrap();
        assert_eq!(
            award,
            TrackedSubmission {
                kind: PendingKind::Unlock,
                id: 101,
                hardcore: false,
                game_hash: "abc".into(),
                score: None
            }
        );
        let submit = track(&request("r=submitlbentry&u=T&t=x&i=7&s=-12&m=abc&v=1")).unwrap();
        assert_eq!((submit.kind, submit.id), (PendingKind::Leaderboard, 7));
        assert_eq!(submit.score, Some(-12));
        assert!(submit.hardcore);
        assert!(
            track(&request("r=awardachievement&a=1&h=1&m=abc"))
                .unwrap()
                .hardcore
        );
        assert_eq!(track(&request("r=ping&g=1&m=abc")), None);
        assert_eq!(track(&request("r=awardachievement&a=x&m=abc")), None);
    }

    #[test]
    fn answers_are_classified_in_order() {
        let observer = RequestObserver::default();
        let tracked = track(&request("r=awardachievement&a=5&h=0&m=abc")).unwrap();
        observer.sent(&tracked);
        let ok = ServerResponse {
            status: 200,
            body: "{\"Success\":true,\"Score\":1,\"AchievementID\":5}".into(),
        };
        observer.answered(tracked.clone(), &ok);
        let log = observer.drain();
        assert_eq!(log[0], RequestObservation::Sent(tracked.clone()));
        assert_eq!(
            log[1],
            RequestObservation::Answered(tracked, ReplayOutcome::Delivered)
        );
        assert!(observer.drain().is_empty());
    }
}
