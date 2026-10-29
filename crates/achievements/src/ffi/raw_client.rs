//! Owner of one `rc_client_t` and its boxed [`ClientState`] userdata. Safe methods
//! over the raw calls; `!Send` (raw pointer), so every call stays on one thread. Drop
//! (or [`RawClient::destroy`]) calls `rc_client_destroy` before the state is freed.

use std::ffi::{c_char, c_int, c_void};
use std::ptr::NonNull;

use super::c_text::{c_string, owned_text};
use super::client_ffi::*;
use super::client_trampolines as tramp;
use super::client_types::RcClient;
use super::raw_hash::client_hash_callbacks;
use crate::achievement_events::ClientOp;
use crate::client_state::ClientState;

pub struct RawClient {
    ptr: NonNull<RcClient>,
    state: Option<Box<ClientState>>,
}

impl RawClient {
    /// Creates the client, wires every trampoline and points userdata at `state`.
    /// `None` only if rcheevos cannot allocate.
    pub fn create(state: Box<ClientState>) -> Option<Self> {
        // SAFETY: valid trampolines; the returned client is checked for NULL.
        let ptr =
            NonNull::new(unsafe { rc_client_create(tramp::read_memory, tramp::server_call) })?;
        let raw = Self {
            ptr,
            state: Some(state),
        };
        let userdata = raw.state() as *const ClientState as *mut c_void;
        // SAFETY: live client; the boxed state outlives it (destroyed first in Drop).
        unsafe {
            rc_client_set_userdata(raw.ptr(), userdata);
            rc_client_enable_logging(raw.ptr(), LOG_LEVEL_WARN, Some(tramp::on_log));
            rc_client_set_event_handler(raw.ptr(), Some(tramp::on_event));
            rc_client_set_hardcore_enabled(raw.ptr(), 0);
            // Copied by rc_client: Rust file / disc-image readers for every hash.
            rc_client_set_hash_callbacks(raw.ptr(), &client_hash_callbacks());
        }
        Some(raw)
    }

    fn ptr(&self) -> *mut RcClient {
        self.ptr.as_ptr()
    }

    /// The userdata state (always present until `destroy`).
    pub fn state(&self) -> &ClientState {
        match &self.state {
            Some(state) => state,
            None => unreachable!("state is only taken by destroy, which consumes self"),
        }
    }

    pub fn set_hardcore_enabled(&self, enabled: bool) {
        // SAFETY: live client on its thread.
        unsafe { rc_client_set_hardcore_enabled(self.ptr(), c_int::from(enabled)) }
    }

    pub fn hardcore_enabled(&self) -> bool {
        // SAFETY: live client.
        unsafe { rc_client_get_hardcore_enabled(self.ptr()) != 0 }
    }

    /// `rcheevos/<major>.<minor>[.<patch>]` (D2: never a hard-coded literal).
    pub fn user_agent_clause(&self) -> String {
        let mut buffer = [0 as c_char; 64];
        // SAFETY: live client; buffer size passed; output NUL-terminated.
        unsafe {
            rc_client_get_user_agent_clause(self.ptr(), buffer.as_mut_ptr(), buffer.len());
            owned_text(buffer.as_ptr())
        }
    }

    fn userdata(op: ClientOp) -> *mut c_void {
        op.as_tag() as *mut c_void
    }

    pub fn begin_login_with_token(&self, username: &str, token: &str) {
        let (user, token) = (c_string(username), c_string(token));
        let tag = Self::userdata(ClientOp::LoginWithToken);
        // SAFETY: live client; strings copied by rc_client before return; tag is an
        // integer, never dereferenced.
        unsafe {
            rc_client_begin_login_with_token(
                self.ptr(),
                user.as_ptr(),
                token.as_ptr(),
                tramp::on_client_callback,
                tag,
            )
        };
    }

    pub fn begin_identify_and_load_game(&self, console_id: u32, path: &str, data: Option<Vec<u8>>) {
        let path = c_string(path);
        let tag = Self::userdata(ClientOp::LoadGame);
        let mut buffers = self.state().hash_buffers.borrow_mut();
        let (data_ptr, data_len) = match data {
            Some(bytes) => {
                buffers.push(bytes);
                buffers
                    .last()
                    .map_or((std::ptr::null(), 0), |b| (b.as_ptr(), b.len()))
            }
            None => (std::ptr::null(), 0),
        };
        drop(buffers);
        // SAFETY: live client; path copied (strdup) by rc_hash; the data buffer lives in
        // `hash_buffers` until the client is destroyed.
        unsafe {
            rc_client_begin_identify_and_load_game(
                self.ptr(),
                console_id,
                path.as_ptr(),
                data_ptr,
                data_len,
                tramp::on_client_callback,
                tag,
            )
        };
    }

    pub fn begin_load_game(&self, hash: &str) {
        let hash = c_string(hash);
        let tag = Self::userdata(ClientOp::LoadGame);
        // SAFETY: live client; hash copied by rc_client.
        unsafe {
            rc_client_begin_load_game(self.ptr(), hash.as_ptr(), tramp::on_client_callback, tag)
        };
    }

    pub fn begin_change_media(&self, hash: &str) {
        let hash = c_string(hash);
        let tag = Self::userdata(ClientOp::ChangeMedia);
        // SAFETY: live client; hash copied by rc_client.
        unsafe {
            rc_client_begin_change_media(self.ptr(), hash.as_ptr(), tramp::on_client_callback, tag)
        };
    }

    pub fn load_game_state(&self) -> i32 {
        // SAFETY: live client.
        unsafe { rc_client_get_load_game_state(self.ptr()) }
    }

    pub fn is_game_loaded(&self) -> bool {
        // SAFETY: live client.
        unsafe { rc_client_is_game_loaded(self.ptr()) != 0 }
    }

    pub fn unload_game(&self) {
        // SAFETY: live client on its thread.
        unsafe { rc_client_unload_game(self.ptr()) }
    }

    /// Destroys the C client and hands the state back (its queue may still own the
    /// HTTP worker, which the caller joins).
    pub fn destroy(mut self) -> Box<ClientState> {
        self.destroy_client();
        match self.state.take() {
            Some(state) => state,
            None => unreachable!("state present until destroy"),
        }
    }

    fn destroy_client(&mut self) {
        // SAFETY: created by rc_client_create; destroyed exactly once (destroy consumes
        // self, Drop runs only when destroy did not).
        unsafe { rc_client_destroy(self.ptr()) };
    }

    pub(super) fn raw(&self) -> *mut RcClient {
        self.ptr()
    }
}

impl Drop for RawClient {
    fn drop(&mut self) {
        if self.state.is_some() {
            self.destroy_client();
        }
    }
}
