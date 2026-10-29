//! "Has the core booted?" for re-query cores (PPSSPP), used by `startup_state_load`.

use std::time::{Duration, Instant};

/// Longest a re-query core may take to boot before the load is tried anyway. Wall
/// clock, not frames: the boot runs on the core's own thread, and headless frames
/// are not paced.
pub const BOOT_WAIT: Duration = Duration::from_secs(10);

/// The core booted when its state size differs from the first one seen (PPSSPP
/// reports a constant 128 MiB until boot), or [`BOOT_WAIT`] passed.
#[derive(Debug, Default)]
pub struct BootGate {
    first: Option<(usize, Instant)>,
    open: bool,
}

impl BootGate {
    /// Feeds the current state size; true from the first booted call on.
    pub fn passed(&mut self, size: usize, now: Instant) -> bool {
        if self.open {
            return true;
        }
        let (first, since) = *self.first.get_or_insert((size, now));
        self.open = size != first || now.duration_since(since) >= BOOT_WAIT;
        if self.open && size == first {
            tracing::warn!("core state size unchanged after {BOOT_WAIT:?}; loading anyway");
        }
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRE_BOOT: usize = 128 << 20;

    #[test]
    fn gate_opens_when_the_state_size_changes() {
        let (mut gate, t0) = (BootGate::default(), Instant::now());
        assert!(!gate.passed(PRE_BOOT, t0));
        assert!(!gate.passed(PRE_BOOT, t0 + Duration::from_millis(100)));
        assert!(gate.passed(40 << 20, t0 + Duration::from_millis(200)));
        assert!(
            gate.passed(PRE_BOOT, t0 + Duration::from_millis(300)),
            "stays open"
        );
    }

    #[test]
    fn gate_opens_after_the_boot_wait_without_a_change() {
        let (mut gate, t0) = (BootGate::default(), Instant::now());
        assert!(!gate.passed(PRE_BOOT, t0));
        assert!(!gate.passed(PRE_BOOT, t0 + BOOT_WAIT - Duration::from_millis(1)));
        assert!(gate.passed(PRE_BOOT, t0 + BOOT_WAIT));
    }
}
