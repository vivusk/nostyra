//! Client-free rebuild + classification of offline journal replays: award and
//! leaderboard submit requests (`o=` = seconds since the event) and their parsers.

use std::mem::MaybeUninit;

use super::api_calls::{build, outcome, with_c_response, ApiFailure};
use super::api_ffi::*;
use super::c_text::c_string;
use crate::server_transport::{ServerRequest, ServerResponse};

/// `r=awardachievement` rebuilt from journal facts (`o=` = seconds since the unlock).
pub fn award_request(
    username: &str,
    token: &str,
    achievement_id: u32,
    hardcore: bool,
    game_hash: &str,
    seconds_since_unlock: u32,
) -> Result<ServerRequest, i32> {
    let (user, token, hash) = (c_string(username), c_string(token), c_string(game_hash));
    let params = RcApiAwardRequest {
        username: user.as_ptr(),
        api_token: token.as_ptr(),
        achievement_id,
        hardcore: u32::from(hardcore),
        game_hash: hash.as_ptr(),
        seconds_since_unlock,
    };
    // SAFETY: params and their strings outlive the call.
    build(|r| unsafe { rc_api_init_award_achievement_request(r, &params) })
}

/// Parses an award answer; "User already has …" counts as success (rcheevos rule).
pub fn parse_award_response(response: &ServerResponse) -> Result<(), ApiFailure> {
    let mut parsed: Box<MaybeUninit<RcApiAwardResponse>> = Box::new(MaybeUninit::zeroed());
    // SAFETY: the parser memsets + fills the boxed response; read, then destroyed once.
    unsafe {
        let rc = with_c_response(response, |server| {
            rc_api_process_award_achievement_server_response(parsed.as_mut_ptr(), server)
        });
        let p = parsed.assume_init_mut();
        let result = outcome(rc, &p.response);
        rc_api_destroy_award_achievement_response(p);
        result
    }
}

/// `r=submitlbentry` rebuilt from journal facts (`o=` = seconds since completion).
pub fn submit_request(
    username: &str,
    token: &str,
    leaderboard_id: u32,
    score: i32,
    game_hash: &str,
    seconds_since_completion: u32,
) -> Result<ServerRequest, i32> {
    let (user, token, hash) = (c_string(username), c_string(token), c_string(game_hash));
    let params = RcApiSubmitRequest {
        username: user.as_ptr(),
        api_token: token.as_ptr(),
        leaderboard_id,
        score,
        game_hash: hash.as_ptr(),
        seconds_since_completion,
    };
    // SAFETY: params and their strings outlive the call.
    build(|r| unsafe { rc_api_init_submit_lboard_entry_request(r, &params) })
}

/// Parses a leaderboard submit answer.
pub fn parse_submit_response(response: &ServerResponse) -> Result<(), ApiFailure> {
    let mut parsed: Box<MaybeUninit<RcApiSubmitResponse>> = Box::new(MaybeUninit::zeroed());
    // SAFETY: the parser memsets + fills the boxed response; read, then destroyed once.
    unsafe {
        let rc = with_c_response(response, |server| {
            rc_api_process_submit_lboard_entry_server_response(parsed.as_mut_ptr(), server)
        });
        let p = parsed.assume_init_mut();
        let result = outcome(rc, &p.response);
        rc_api_destroy_submit_lboard_entry_response(p);
        result
    }
}
