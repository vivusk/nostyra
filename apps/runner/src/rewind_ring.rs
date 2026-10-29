//! Rewind history: a memory-capped ring of compressed save-state deltas (pure, no
//! core dependency).
//!
//! Keeps the newest raw state (`current`) plus one entry per older capture. An
//! entry is the LZ4 block of `newer XOR older`: consecutive states are mostly
//! identical, so the XOR is mostly zeros and compresses to a few KiB. Popping XORs
//! the decompressed newest entry into `current`, which walks back one capture.
//! Pushing after pops continues from the rewound point (history beyond it is gone,
//! as in RetroArch). Each capture may carry a small raw achievement-progress blob
//! (M15, only while an achievement session runs), counted in the budget.

use std::collections::VecDeque;

use lz4_flex::block::CompressTable;
use lz4_flex::block::{compress_into_with_table, decompress_into, get_maximum_output_size};

pub(crate) struct RewindRing {
    /// Bytes allowed for `current` + entries (scratch buffers, ~2× a state, not counted).
    budget: usize,
    /// Newest raw state; empty until the first push.
    current: Vec<u8>,
    /// Oldest first; each turns the state after it (or `current`) into its own state.
    entries: VecDeque<Box<[u8]>>,
    /// Achievement progress of `current`, and of each entry's capture (same order).
    current_progress: Option<Box<[u8]>>,
    progress: VecDeque<Option<Box<[u8]>>>,
    entry_bytes: usize,
    /// Reused XOR / decompression buffer (state size).
    delta: Vec<u8>,
    /// Reused compression output (worst-case LZ4 size); each entry copies its prefix.
    packed: Vec<u8>,
    table: CompressTable,
}

impl RewindRing {
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self {
            budget: budget_bytes,
            current: Vec::new(),
            entries: VecDeque::new(),
            current_progress: None,
            progress: VecDeque::new(),
            entry_bytes: 0,
            delta: Vec::new(),
            packed: Vec::new(),
            table: CompressTable::large(),
        }
    }

    /// Records `state` as the newest capture. A size change (variable-size core)
    /// restarts the history from `state`.
    #[cfg(test)]
    pub(crate) fn push(&mut self, state: &[u8]) {
        self.push_with_progress(state, None);
    }

    /// [`Self::push`] with the capture's achievement progress.
    pub(crate) fn push_with_progress(&mut self, state: &[u8], progress: Option<&[u8]>) {
        let progress = progress.map(Box::<[u8]>::from);
        if self.current.len() != state.len() {
            if !self.current.is_empty() {
                tracing::debug!(
                    from = self.current.len(),
                    to = state.len(),
                    "state size changed; rewind history cleared"
                );
            }
            self.clear();
            self.current.extend_from_slice(state);
            self.entry_bytes += blob_len(&progress);
            self.current_progress = progress;
            return;
        }
        self.delta.clear();
        self.delta
            .extend(state.iter().zip(&self.current).map(|(new, old)| new ^ old));
        self.packed
            .resize(get_maximum_output_size(self.delta.len()), 0);
        match compress_into_with_table(&self.delta, &mut self.packed, &mut self.table) {
            Ok(len) => {
                let entry: Box<[u8]> = self.packed[..len].into();
                self.entry_bytes += entry.len() + blob_len(&progress);
                self.entries.push_back(entry);
                let older = std::mem::replace(&mut self.current_progress, progress);
                self.progress.push_back(older);
            }
            // Cannot happen with a worst-case sized output; restart rather than keep a gap.
            Err(err) => {
                tracing::warn!(%err, "rewind delta compression failed; history cleared");
                self.clear();
            }
        }
        self.current.copy_from_slice(state);
        self.evict_over_budget();
    }

    /// Steps back one capture and returns that state; `None` when no older capture
    /// is left (`current` stays the oldest one) or an entry was corrupt (history
    /// cleared, so garbage is never handed to the core).
    pub(crate) fn pop(&mut self) -> Option<&[u8]> {
        let entry = self.entries.pop_back()?;
        self.entry_bytes -= entry.len() + blob_len(&self.current_progress);
        self.current_progress = self.progress.pop_back().flatten();
        self.delta.resize(self.current.len(), 0);
        match decompress_into(&entry, &mut self.delta) {
            Ok(len) if len == self.current.len() => {}
            _ => {
                tracing::warn!("corrupt rewind entry; history cleared");
                self.clear();
                return None;
            }
        }
        for (state, delta) in self.current.iter_mut().zip(&self.delta) {
            *state ^= delta;
        }
        Some(&self.current)
    }

    /// Newest state (after pops: the one rewound to); `None` before the first push.
    pub(crate) fn current(&self) -> Option<&[u8]> {
        (!self.current.is_empty()).then_some(self.current.as_slice())
    }

    /// Achievement progress of the capture [`Self::current`] returns.
    pub(crate) fn current_progress(&self) -> Option<&[u8]> {
        self.current_progress.as_deref()
    }

    /// Forgets all history (slot load, core reset, size change).
    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.entries.clear();
        self.current_progress = None;
        self.progress.clear();
        self.entry_bytes = 0;
    }

    /// Delta entries held (= captures that can be popped).
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Bytes counted against the budget: `current` plus all entries.
    pub(crate) fn bytes_used(&self) -> usize {
        self.current.len() + self.entry_bytes
    }

    fn evict_over_budget(&mut self) {
        while self.bytes_used() > self.budget {
            let Some(oldest) = self.entries.pop_front() else {
                break;
            };
            let progress = self.progress.pop_front().flatten();
            self.entry_bytes -= oldest.len() + blob_len(&progress);
        }
    }

    /// Test hook: the newest entry's bytes (to corrupt it).
    #[cfg(test)]
    fn newest_entry_mut(&mut self) -> Option<&mut Box<[u8]>> {
        self.entries.back_mut()
    }
}

/// Budget bytes of an optional progress blob.
fn blob_len(blob: &Option<Box<[u8]>>) -> usize {
    blob.as_ref().map_or(0, |b| b.len())
}

#[cfg(test)]
#[path = "rewind_ring_tests.rs"]
mod tests;
