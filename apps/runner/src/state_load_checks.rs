//! Checks around loading a state file into the core: the per-core size cap
//! (`CoreSpec::max_state_bytes`) and the snapshot that puts the game back when the
//! core rejects a state.

use anyhow::bail;
use ipc_protocol::StateSlotId;
use libretro_host::{CoreSession, HostResult};

/// Refuses a state file larger than the core's cap before it is read into memory.
pub(crate) fn check_state_size(id: StateSlotId, size: u64, max: u64) -> anyhow::Result<()> {
    if size > max {
        bail!("state in slot {id} is {size} bytes, over the {max}-byte limit");
    }
    Ok(())
}

/// What `unserialize_or_restore` needs from a session (a fake in tests).
pub(crate) trait StateTarget {
    fn serialize(&mut self) -> HostResult<Vec<u8>>;
    fn unserialize(&mut self, state: &[u8]) -> HostResult<()>;
}

impl StateTarget for CoreSession {
    fn serialize(&mut self) -> HostResult<Vec<u8>> {
        CoreSession::serialize(self)
    }

    fn unserialize(&mut self, state: &[u8]) -> HostResult<()> {
        CoreSession::unserialize(self, state)
    }
}

/// Loads `state`; if the core rejects it, re-applies a snapshot taken just before.
/// Flycast leaves its emulator stopped after a rejected state (M11 phase-00 spike);
/// the snapshot puts the game back where it was instead of a frozen image. A core
/// that cannot serialize yet (PPSSPP before boot) gets the load without a snapshot.
pub(crate) fn unserialize_or_restore(
    session: &mut impl StateTarget,
    state: &[u8],
) -> anyhow::Result<()> {
    let before = session.serialize().ok();
    let Err(err) = session.unserialize(state) else {
        return Ok(());
    };
    if let Some(before) = before {
        match session.unserialize(&before) {
            Ok(()) => tracing::warn!("state rejected; the game continues where it was"),
            Err(again) => tracing::error!(%again, "state rejected and the snapshot too"),
        }
    }
    Err(err.into())
}

#[cfg(test)]
mod tests {
    use libretro_host::HostError;

    use super::*;

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn size_cap_follows_the_core() {
        let slot = StateSlotId::Numbered(1);
        assert!(check_state_size(slot, 100 * MIB, 128 * MIB).is_ok());
        assert!(check_state_size(slot, 100 * MIB, 64 * MIB).is_err());
        assert!(
            check_state_size(slot, 64 * MIB, 64 * MIB).is_ok(),
            "cap inclusive"
        );
    }

    /// Records unserialize calls; serialize and unserialize results are scripted.
    struct Fake {
        serialize_ok: bool,
        accepts: fn(&[u8]) -> bool,
        loaded: Vec<Vec<u8>>,
    }

    impl StateTarget for Fake {
        fn serialize(&mut self) -> HostResult<Vec<u8>> {
            match self.serialize_ok {
                true => Ok(b"snapshot".to_vec()),
                false => Err(HostError::SerializeFailed),
            }
        }

        fn unserialize(&mut self, state: &[u8]) -> HostResult<()> {
            self.loaded.push(state.to_vec());
            (self.accepts)(state)
                .then_some(())
                .ok_or(HostError::UnserializeFailed)
        }
    }

    fn fake(serialize_ok: bool, accepts: fn(&[u8]) -> bool) -> Fake {
        Fake {
            serialize_ok,
            accepts,
            loaded: Vec::new(),
        }
    }

    #[test]
    fn failed_snapshot_does_not_block_the_load() {
        let mut core = fake(false, |_| true);
        unserialize_or_restore(&mut core, b"state").unwrap();
        assert_eq!(core.loaded, [b"state".to_vec()]);
    }

    #[test]
    fn rejected_state_restores_the_snapshot() {
        let mut core = fake(true, |s| s == b"snapshot");
        assert!(unserialize_or_restore(&mut core, b"bad").is_err());
        assert_eq!(core.loaded, [b"bad".to_vec(), b"snapshot".to_vec()]);
    }

    #[test]
    fn rejected_state_without_a_snapshot_is_just_an_error() {
        let mut core = fake(false, |_| false);
        assert!(unserialize_or_restore(&mut core, b"bad").is_err());
        assert_eq!(core.loaded, [b"bad".to_vec()]);
    }
}
