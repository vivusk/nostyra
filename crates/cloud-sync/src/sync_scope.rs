//! Which files of a unit take part in sync: one predicate, applied to the local
//! snapshot, the base and the remote manifest alike (`sync_scope_maps`), so a path
//! out of scope is never pushed, pulled or tombstoned on either side — a states
//! toggle switched off on one PC never deletes states on another.

use ipc_protocol::{
    is_browser_swap_file, StateSlotId, MAX_STATE_SLOT, STATE_SLOT_FILE_EXTS, WEB_STATE_PREFIX,
};

/// Files larger than this are never uploaded or downloaded (skipped + logged).
pub const MAX_SYNC_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Dolphin's shader / texture cache inside the save folder: rebuilt on demand.
const EXCLUDED_DIR: [&str; 2] = ["User", "Cache"];

/// The user's sync settings that shape the scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncScope {
    /// "Also sync save states": `slotN.*` / `auto.*` (desktop and `web-` stems).
    pub sync_states: bool,
    /// Per-file size cap ([`MAX_SYNC_FILE_BYTES`]; smaller in tests).
    pub max_file_bytes: u64,
}

impl Default for SyncScope {
    fn default() -> Self {
        Self {
            sync_states: true,
            max_file_bytes: MAX_SYNC_FILE_BYTES,
        }
    }
}

impl SyncScope {
    /// The one scope rule: `path` (relative, `/`-separated) of `size` bytes syncs.
    pub fn in_scope(&self, path: &str, size: u64) -> bool {
        !self.excluded_path(path) && !self.over_cap(size)
    }

    /// Whether a file of `size` bytes is over the per-file cap.
    pub fn over_cap(&self, size: u64) -> bool {
        size > self.max_file_bytes
    }

    /// Path-only part of the scope (exclusions + states toggle).
    pub fn excluded_path(&self, path: &str) -> bool {
        if is_excluded_dir(path) {
            return true;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tmp")
            || is_browser_swap_file(name)
            || lower.contains(".before-import-")
            || lower.contains(".conflict-")
        {
            return true;
        }
        !self.sync_states && !path.contains('/') && is_state_slot_file(name)
    }
}

/// `User/Cache` itself or anything below it (case-insensitive, as on Windows).
pub fn is_excluded_dir(path: &str) -> bool {
    let mut parts = path.split('/');
    EXCLUDED_DIR.iter().all(|want| {
        parts
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case(want))
    })
}

/// A save-state slot file at the folder root: `<stem>.<state|json|png>` where the
/// stem is a [`StateSlotId`] stem, optionally with the web edition's prefix.
fn is_state_slot_file(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    if !STATE_SLOT_FILE_EXTS.contains(&ext) {
        return false;
    }
    let stem = stem.strip_prefix(WEB_STATE_PREFIX).unwrap_or(stem);
    (0..=MAX_STATE_SLOT)
        .map(StateSlotId::Numbered)
        .chain([StateSlotId::Auto])
        .any(|id| id.file_stem() == stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusions_and_states_table() {
        let on = SyncScope::default();
        let off = SyncScope {
            sync_states: false,
            ..on
        };
        // (path, in scope with states on, with states off)
        let table = [
            ("save.srm", true, true),
            ("save.web.srm", true, true),
            ("rtc.bin", true, true),
            ("content.eeprom", true, true),
            ("PSP/SAVEDATA/ULUS10041/DATA.BIN", true, true),
            ("User/GC/USA/Card A/x.gci", true, true),
            ("User/Cache/Shaders/a.cache", false, false),
            ("user/cache/x", false, false),
            ("User/Cachet/x", true, true),
            ("save.srm.tmp", false, false),
            ("save.srm.crswap", false, false),
            ("save.srm.before-import-261109-101010", false, false),
            ("save.srm.conflict-261109", false, false),
            ("slot1.state", true, false),
            ("slot9.png", true, false),
            ("auto.json", true, false),
            ("web-slot1.state", true, false),
            ("web-auto.state", true, false),
            ("slot10.state", true, true),
            ("slot1.srm", true, true),
            ("User/auto.json", true, true),
        ];
        for (path, with_states, without) in table {
            assert_eq!(on.in_scope(path, 1), with_states, "{path} states on");
            assert_eq!(off.in_scope(path, 1), without, "{path} states off");
        }
    }

    #[test]
    fn size_cap() {
        let scope = SyncScope::default();
        assert!(scope.in_scope("save.srm", MAX_SYNC_FILE_BYTES));
        assert!(!scope.in_scope("save.srm", MAX_SYNC_FILE_BYTES + 1));
        assert!(is_excluded_dir("User/Cache") && !is_excluded_dir("User"));
    }
}
