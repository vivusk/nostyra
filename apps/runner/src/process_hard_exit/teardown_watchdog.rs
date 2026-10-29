//! Teardown watchdog: a core that never finishes unloading (Dolphin #520: `retro_unload_game`
//! may hang) must not leave a runner process hanging after the game ended. Armed when
//! the session teardown starts (a [`TeardownGuard`] dropping right before the value
//! that owns the session), disarmed by `main` once the run returned. If the teardown
//! takes longer than [`TEARDOWN_TIMEOUT`], the fire function reports the session
//! (`SessionReporter::finished` with [`TEARDOWN_TIMED_OUT`]: IPC `Stopped` + pending
//! play-session record when it had started) and ends the process with the run's own
//! exit code (`exit_now`).
//!
//! The watchdog is a plain value with an injected fire function (tests pass a
//! recorder; `main` passes [`TeardownWatchdog::reporting`]); nothing is process-global.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use crate::session_reporter::SessionReporter;

/// Longest a session teardown may take (spike: Dolphin unloads in 195–425 ms).
pub const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(10);
/// `finished` error text when the watchdog ended the process.
pub const TEARDOWN_TIMED_OUT: &str = "core teardown timed out";

/// Debug-build test hooks: override the timeout / slow the teardown down (ms).
#[cfg(debug_assertions)]
const TIMEOUT_ENV: &str = "NOSTYRA_TEST_TEARDOWN_TIMEOUT_MS";
#[cfg(debug_assertions)]
const DELAY_ENV: &str = "NOSTYRA_TEST_TEARDOWN_DELAY_MS";

/// Cheap to clone; every clone is the same watchdog.
#[derive(Clone)]
pub struct TeardownWatchdog {
    inner: Arc<Inner>,
}

struct Inner {
    timeout: Duration,
    /// Set by the first `arm` (later ones are no-ops).
    armed: AtomicBool,
    disarmed: Mutex<bool>,
    wake: Condvar,
    fire: Box<dyn Fn(u8) + Send + Sync>,
}

impl TeardownWatchdog {
    pub fn new(timeout: Duration, fire: impl Fn(u8) + Send + Sync + 'static) -> Self {
        let inner = Inner {
            timeout,
            armed: AtomicBool::new(false),
            disarmed: Mutex::new(false),
            wake: Condvar::new(),
            fire: Box::new(fire),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// The runner's watchdog: on timeout reports through `reporter`, then exits.
    pub fn reporting(reporter: &SessionReporter) -> Self {
        let reporter = reporter.clone();
        Self::new(timeout(), move |code| {
            reporter.finished(code, Some(TEARDOWN_TIMED_OUT.to_owned()));
            super::exit_now(code)
        })
    }

    /// Starts the countdown (once; later calls do nothing): fires with `code` unless
    /// disarmed within the timeout.
    pub fn arm(&self, code: u8) {
        if self.inner.armed.swap(true, Ordering::SeqCst) {
            return;
        }
        let inner = Arc::clone(&self.inner);
        let spawned = std::thread::Builder::new()
            .name("teardown-watchdog".into())
            .spawn(move || inner.watch(code));
        if let Err(err) = spawned {
            tracing::warn!(%err, "cannot start the teardown watchdog");
        }
    }

    /// The teardown finished: never fire. Waits for a fire already under way (which
    /// ends the process in production), so the two never both report.
    pub fn disarm(&self) {
        *self
            .inner
            .disarmed
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.inner.wake.notify_all();
    }

    /// A guard that arms with `code` when dropped.
    pub fn guard(&self, code: u8) -> TeardownGuard {
        TeardownGuard {
            watchdog: self.clone(),
            code,
        }
    }
}

impl Inner {
    fn watch(&self, code: u8) {
        let disarmed = self.disarmed.lock().unwrap_or_else(PoisonError::into_inner);
        let (disarmed, _) = self
            .wake
            .wait_timeout_while(disarmed, self.timeout, |disarmed| !*disarmed)
            .unwrap_or_else(PoisonError::into_inner);
        if !*disarmed {
            tracing::warn!(code, timeout = ?self.timeout, "{TEARDOWN_TIMED_OUT}");
            (self.fire)(code); // lock held: a late `disarm` waits for the report
        }
    }
}

/// Arms its watchdog when dropped: declare it right after the value whose drop is the
/// teardown (locals drop in reverse order, so the guard arms first).
#[must_use = "the watchdog arms when the guard drops"]
pub struct TeardownGuard {
    watchdog: TeardownWatchdog,
    code: u8,
}

impl TeardownGuard {
    /// The same guard arming with `code` (the run's result is known now).
    pub fn with_code(mut self, code: u8) -> Self {
        self.code = code;
        self
    }
}

impl Drop for TeardownGuard {
    fn drop(&mut self) {
        self.watchdog.arm(self.code);
    }
}

/// [`TEARDOWN_TIMEOUT`], or `NOSTYRA_TEST_TEARDOWN_TIMEOUT_MS` in a debug build.
fn timeout() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(ms) = env_ms(TIMEOUT_ENV) {
        return Duration::from_millis(ms);
    }
    TEARDOWN_TIMEOUT
}

#[cfg(debug_assertions)]
fn env_ms(key: &str) -> Option<u64> {
    std::env::var(key).ok()?.parse().ok()
}

/// Debug-build test hook: sleeps `NOSTYRA_TEST_TEARDOWN_DELAY_MS` when dropped, to
/// stand in for a hanging unload (declared before the guard: drops after it armed).
pub struct TestTeardownDelay;

impl Drop for TestTeardownDelay {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(ms) = env_ms(DELAY_ENV) {
            tracing::warn!(ms, "test hook: slow teardown");
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
}

#[cfg(test)]
#[path = "teardown_watchdog_tests.rs"]
mod tests;
