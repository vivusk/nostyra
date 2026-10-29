//! State the C trampolines reach through `rc_client_get_userdata`: the memory reader,
//! the request queue, and the event / completion queues the session drains. Boxed and
//! owned by the raw client, so its address is stable for the client's whole life.

use std::cell::RefCell;

use crate::achievement_events::{AchievementEvent, ClientCompletion};
use crate::request_queue::RequestQueue;

/// Reads emulated memory for rc_client: `(address, buffer) → bytes read` (0 = none).
/// Phase 02 plugs `rc_libretro` regions in; tests use a plain byte vector.
pub type MemoryReader = Box<dyn FnMut(u32, &mut [u8]) -> u32>;

/// Userdata of one rc_client (client thread only).
pub(crate) struct ClientState {
    pub(crate) memory: RefCell<MemoryReader>,
    pub(crate) queue: RequestQueue,
    pub(crate) events: RefCell<Vec<AchievementEvent>>,
    pub(crate) completions: RefCell<Vec<ClientCompletion>>,
    /// Buffers handed to `begin_identify_and_load_game`: rc_hash keeps a pointer to
    /// them in the load state (re-iterates on an unknown hash after the server reply),
    /// so they live as long as the client.
    pub(crate) hash_buffers: RefCell<Vec<Vec<u8>>>,
}

impl ClientState {
    pub(crate) fn new(memory: MemoryReader, queue: RequestQueue) -> Self {
        Self {
            memory: RefCell::new(memory),
            queue,
            events: RefCell::new(Vec::new()),
            completions: RefCell::new(Vec::new()),
            hash_buffers: RefCell::new(Vec::new()),
        }
    }

    /// Memory read from rc_client; a re-entrant call (never expected) reads nothing.
    pub(crate) fn read_memory(&self, address: u32, buffer: &mut [u8]) -> u32 {
        match self.memory.try_borrow_mut() {
            Ok(mut reader) => reader(address, buffer).min(buffer.len() as u32),
            Err(_) => 0,
        }
    }

    pub(crate) fn push_event(&self, event: AchievementEvent) {
        self.events.borrow_mut().push(event);
    }

    pub(crate) fn push_completion(&self, completion: ClientCompletion) {
        self.completions.borrow_mut().push(completion);
    }
}
