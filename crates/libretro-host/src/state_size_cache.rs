//! Save-state size per savestate context. Cores may answer `retro_serialize_size`
//! differently while `FAST_SAVESTATES` is set (run-ahead / rewind context): ProSystem
//! 49 221 B Normal / 83 968 B fast, and it refuses a buffer of the other size; Stella
//! 1 MiB fast. So the size is queried **inside** the context of the serialize it is
//! for and cached per context (M17 spike). Cores that ignore the bit get the same
//! size twice: no behaviour change for them. Tests: `core_session_state_size_tests`.

use crate::core_session::CoreSession;
use crate::host_error::HostResult;
use crate::savestate_context::SavestateContext;
use crate::serialization_quirks::resolve_state_size;

/// Cached state size of each context (`None`: not queried yet, or variable size).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StateSizes {
    normal: Option<usize>,
    fast: Option<usize>,
}

impl StateSizes {
    fn slot(&mut self, context: SavestateContext) -> &mut Option<usize> {
        match context {
            SavestateContext::Normal => &mut self.normal,
            SavestateContext::RunaheadSameInstance => &mut self.fast,
        }
    }
}

impl CoreSession {
    /// State size for a serialize / unserialize in `context`: the cached size of that
    /// context, else `retro_serialize_size` queried with `context` answered to the
    /// core (cached unless the size is variable or re-queried).
    pub(crate) fn state_size_in(&mut self, context: SavestateContext) -> HostResult<usize> {
        let quirks = self.size_quirks();
        let query = self.core.serialize_size;
        let mut cached = *self.state_sizes.slot(context);
        let mut size = Ok(0);
        self.with_context(context, || {
            // SAFETY: plain query; game loaded.
            size = resolve_state_size(&mut cached, quirks, || unsafe { query() });
            true
        });
        *self.state_sizes.slot(context) = cached;
        size
    }
}
