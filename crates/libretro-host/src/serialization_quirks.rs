//! `SET_SERIALIZATION_QUIRKS`: what the core says about its save states, and the
//! state-size rule that follows from it (cached unless the size may change).

use crate::host_error::{HostError, HostResult};

/// State is not reliable for frame-exact features (run-ahead, rewind, netplay).
const INCOMPLETE: u64 = 1 << 0;
/// Serialize fails until the core has run for a while.
const MUST_INITIALIZE: u64 = 1 << 1;
/// `retro_serialize_size` may change during the session.
const CORE_VARIABLE_SIZE: u64 = 1 << 2;
/// Frontend reply: we re-query the size on every serialize of such a core.
const FRONT_VARIABLE_SIZE: u64 = 1 << 3;
/// Informational only (they matter for files on disk, not RAM-only states).
const SINGLE_SESSION: u64 = 1 << 4;
const ENDIAN_DEPENDENT: u64 = 1 << 5;
const PLATFORM_DEPENDENT: u64 = 1 << 6;

/// Quirk bits we understand; anything else is cleared in the reply.
const KNOWN: u64 = INCOMPLETE
    | MUST_INITIALIZE
    | CORE_VARIABLE_SIZE
    | SINGLE_SESSION
    | ENDIAN_DEPENDENT
    | PLATFORM_DEPENDENT;

/// Quirks reported by the core (default: none, i.e. clean fixed-size states).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SerializationQuirks(u64);

impl SerializationQuirks {
    /// Stores the core's bits and returns the reply: known bits kept, unknown ones
    /// cleared, `FRONT_VARIABLE_SIZE` added when the core's size is variable.
    pub(crate) fn negotiate(core_bits: u64) -> (Self, u64) {
        let quirks = Self(core_bits & KNOWN);
        let mut reply = quirks.0;
        if quirks.variable_size() {
            reply |= FRONT_VARIABLE_SIZE;
        }
        (quirks, reply)
    }

    /// States are incomplete: run-ahead/rewind must stay off.
    pub fn incomplete(self) -> bool {
        self.0 & INCOMPLETE != 0
    }

    /// Serialize may fail early in the session; callers retry later.
    pub fn must_initialize(self) -> bool {
        self.0 & MUST_INITIALIZE != 0
    }

    /// State size may change, so it is queried before every serialize.
    pub fn variable_size(self) -> bool {
        self.0 & CORE_VARIABLE_SIZE != 0
    }

    /// The same quirks with a variable state size (host re-query rule).
    pub(crate) fn with_variable_size(self) -> Self {
        Self(self.0 | CORE_VARIABLE_SIZE)
    }
}

/// Size for the next serialize: `cached` unless the core's size is variable, else a
/// fresh `query()` (cached for fixed-size cores). A zero size means "not yet" for
/// `MUST_INITIALIZE` cores and "no save states" otherwise.
pub(crate) fn resolve_state_size(
    cached: &mut Option<usize>,
    quirks: SerializationQuirks,
    query: impl FnOnce() -> usize,
) -> HostResult<usize> {
    if let Some(size) = cached.filter(|_| !quirks.variable_size()) {
        return Ok(size);
    }
    match query() {
        0 if quirks.must_initialize() => Err(HostError::SerializeNotReady),
        0 => Err(HostError::SerializeUnsupported),
        size => {
            *cached = (!quirks.variable_size()).then_some(size);
            Ok(size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_keeps_known_bits_and_acknowledges_variable_size() {
        let (quirks, reply) = SerializationQuirks::negotiate(INCOMPLETE | (1 << 40));
        assert!(quirks.incomplete());
        assert_eq!(reply, INCOMPLETE, "unknown bit cleared");
        let (quirks, reply) = SerializationQuirks::negotiate(CORE_VARIABLE_SIZE | MUST_INITIALIZE);
        assert!(quirks.variable_size() && quirks.must_initialize());
        assert_eq!(
            reply,
            CORE_VARIABLE_SIZE | MUST_INITIALIZE | FRONT_VARIABLE_SIZE
        );
        // A core cannot claim the frontend-only bit for us.
        assert_eq!(SerializationQuirks::negotiate(FRONT_VARIABLE_SIZE).1, 0);
    }

    #[test]
    fn fixed_size_is_queried_once() {
        let (mut cached, mut queries) = (None, 0);
        for _ in 0..3 {
            let size = resolve_state_size(&mut cached, SerializationQuirks::default(), || {
                queries += 1;
                800
            });
            assert_eq!(size.unwrap(), 800);
        }
        assert_eq!(queries, 1);
    }

    #[test]
    fn variable_size_is_queried_every_time() {
        let quirks = SerializationQuirks::negotiate(CORE_VARIABLE_SIZE).0;
        let mut cached = Some(100);
        assert_eq!(
            resolve_state_size(&mut cached, quirks, || 200).unwrap(),
            200
        );
        assert_eq!(
            resolve_state_size(&mut cached, quirks, || 300).unwrap(),
            300
        );
        assert_eq!(cached, None);
    }

    #[test]
    fn zero_size_is_not_ready_only_for_must_initialize_cores() {
        let quirks = SerializationQuirks::negotiate(MUST_INITIALIZE).0;
        let mut cached = None;
        assert!(matches!(
            resolve_state_size(&mut cached, quirks, || 0),
            Err(HostError::SerializeNotReady)
        ));
        assert!(matches!(
            resolve_state_size(&mut cached, SerializationQuirks::default(), || 0),
            Err(HostError::SerializeUnsupported)
        ));
        assert_eq!(resolve_state_size(&mut cached, quirks, || 64).unwrap(), 64);
    }
}
