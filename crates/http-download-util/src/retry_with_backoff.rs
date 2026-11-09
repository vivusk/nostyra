//! Retry loop shared by network callers: up to `max_tries` attempts, waiting what the
//! server asked (`Retry-After`) or 1, 2, 4 … 32 s plus up to 25 % jitter. An optional
//! deadline (the 15 s pre-play budget) is never slept past: the last error comes back
//! instead. Non-idempotent calls (upload-session creation) only retry when the server
//! explicitly refused with a `Retry-After`, never after an ambiguous failure.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::DownloadError;

/// Attempts per call unless the caller says otherwise.
pub const DEFAULT_MAX_TRIES: u32 = 5;
/// Longest computed backoff step.
pub const MAX_BACKOFF: Duration = Duration::from_secs(32);
/// A `Retry-After` longer than this is not waited out: the error is returned and the
/// caller retries at its next sync instead of holding a worker.
pub const MAX_RETRY_WAIT: Duration = Duration::from_secs(60);

/// What an error says about trying again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryHint {
    /// Retrying cannot help (4xx, auth, local error).
    Stop,
    /// Transient: back off and retry (request may or may not have been processed).
    Retry,
    /// The server refused and named the wait (`Retry-After`).
    RetryAfter(Duration),
}

/// Errors the retry loop can classify.
pub trait RetryClass {
    fn retry_hint(&self) -> RetryHint;
}

impl RetryClass for DownloadError {
    fn retry_hint(&self) -> RetryHint {
        match self {
            Self::RateLimited { retry_after, .. } => RetryHint::RetryAfter(*retry_after),
            Self::Transient(reason) if reason == "offline" => RetryHint::Stop,
            Self::Transient(_) => RetryHint::Retry,
            _ => RetryHint::Stop,
        }
    }
}

/// How hard to retry one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_tries: u32,
    /// Never sleep past this instant (pre-play budget).
    pub deadline: Option<Instant>,
    /// False for calls that must not run twice (only `RetryAfter` refusals retry).
    pub idempotent: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_tries: DEFAULT_MAX_TRIES,
            deadline: None,
            idempotent: true,
        }
    }
}

/// Base wait before retry number `attempt` (1 = first retry): 1, 2, 4 … 32 s.
pub fn backoff_delay(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.saturating_sub(1).min(5);
    Duration::from_secs(secs).min(MAX_BACKOFF)
}

/// Runs `op(attempt)` (attempt starts at 1) under `policy`, sleeping on the thread.
pub fn retry_with_backoff<T, E: RetryClass>(
    policy: RetryPolicy,
    op: impl FnMut(u32) -> Result<T, E>,
) -> Result<T, E> {
    retry_with_clock(policy, op, Instant::now, std::thread::sleep)
}

/// [`retry_with_backoff`] with an injected clock and sleeper (tests).
pub fn retry_with_clock<T, E: RetryClass>(
    policy: RetryPolicy,
    mut op: impl FnMut(u32) -> Result<T, E>,
    mut now: impl FnMut() -> Instant,
    mut sleep: impl FnMut(Duration),
) -> Result<T, E> {
    let mut attempt = 1;
    loop {
        let error = match op(attempt) {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        let wait = match error.retry_hint() {
            RetryHint::Stop => return Err(error),
            RetryHint::Retry if !policy.idempotent => return Err(error),
            RetryHint::Retry => with_jitter(backoff_delay(attempt)),
            RetryHint::RetryAfter(wait) if wait > MAX_RETRY_WAIT => return Err(error),
            RetryHint::RetryAfter(wait) => wait,
        };
        if attempt >= policy.max_tries {
            return Err(error);
        }
        if let Some(deadline) = policy.deadline {
            if now() + wait >= deadline {
                return Err(error);
            }
        }
        tracing::debug!(
            attempt,
            wait_ms = wait.as_millis() as u64,
            "retrying request"
        );
        sleep(wait);
        attempt += 1;
    }
}

/// Adds 0–25 % jitter (clock nanos are random enough to spread retries).
fn with_jitter(base: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    base + base.mul_f64(f64::from(nanos % 1000) / 4000.0)
}

#[cfg(test)]
#[path = "retry_with_backoff_tests.rs"]
mod tests;
