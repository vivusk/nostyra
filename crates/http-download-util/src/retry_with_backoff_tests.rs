use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use super::*;

/// Runs `policy` against `errors` (one per attempt, then success) on a fake clock;
/// returns (result, attempts made, sleeps).
fn run(
    policy: RetryPolicy,
    errors: Vec<DownloadError>,
    start: Instant,
) -> (Result<u32, DownloadError>, u32, Vec<Duration>) {
    let errors = RefCell::new(errors.into_iter());
    let attempts = Cell::new(0);
    let clock = Cell::new(start);
    let sleeps = RefCell::new(Vec::new());
    let result = retry_with_clock(
        policy,
        |attempt| {
            attempts.set(attempt);
            match errors.borrow_mut().next() {
                Some(error) => Err(error),
                None => Ok(attempt),
            }
        },
        || clock.get(),
        |wait| {
            clock.set(clock.get() + wait);
            sleeps.borrow_mut().push(wait);
        },
    );
    (result, attempts.get(), sleeps.into_inner())
}

fn transient() -> DownloadError {
    DownloadError::Transient("http status 502".into())
}

#[test]
fn backoff_schedule_doubles_to_32s() {
    let secs: Vec<u64> = (1..=8).map(|n| backoff_delay(n).as_secs()).collect();
    assert_eq!(secs, [1, 2, 4, 8, 16, 32, 32, 32]);
}

#[test]
fn transient_errors_back_off_with_jitter_then_succeed() {
    let (result, attempts, sleeps) = run(
        RetryPolicy::default(),
        vec![transient(), transient(), transient()],
        Instant::now(),
    );
    assert_eq!(result.ok(), Some(4));
    assert_eq!(attempts, 4);
    for (n, wait) in sleeps.iter().enumerate() {
        let base = backoff_delay(n as u32 + 1);
        assert!(*wait >= base && *wait <= base.mul_f64(1.25), "{wait:?}");
    }
}

#[test]
fn gives_up_after_max_tries() {
    let errors = (0..10).map(|_| transient()).collect();
    let (result, attempts, sleeps) = run(RetryPolicy::default(), errors, Instant::now());
    assert!(result.is_err());
    assert_eq!(attempts, DEFAULT_MAX_TRIES);
    assert_eq!(sleeps.len(), DEFAULT_MAX_TRIES as usize - 1);
}

#[test]
fn honours_retry_after() {
    let limited = DownloadError::RateLimited {
        status: 429,
        retry_after: Duration::from_secs(7),
    };
    let (result, _, sleeps) = run(RetryPolicy::default(), vec![limited], Instant::now());
    assert!(result.is_ok());
    assert_eq!(sleeps, [Duration::from_secs(7)]);
}

#[test]
fn never_sleeps_past_the_deadline() {
    let start = Instant::now();
    let policy = RetryPolicy {
        deadline: Some(start + Duration::from_secs(5)),
        ..RetryPolicy::default()
    };
    let errors = (0..10).map(|_| transient()).collect();
    let (result, attempts, sleeps) = run(policy, errors, start);
    assert!(result.is_err());
    // 1 s + 2 s (+ jitter) fit in 5 s; the 4 s step would not.
    assert_eq!(attempts, 3);
    assert!(sleeps.iter().sum::<Duration>() < Duration::from_secs(5));
}

#[test]
fn non_idempotent_only_retries_explicit_refusals() {
    let policy = RetryPolicy {
        idempotent: false,
        ..RetryPolicy::default()
    };
    let (result, attempts, _) = run(policy, vec![transient()], Instant::now());
    assert!(result.is_err());
    assert_eq!(attempts, 1);
    let limited = DownloadError::RateLimited {
        status: 503,
        retry_after: Duration::from_secs(1),
    };
    let (result, attempts, _) = run(policy, vec![limited], Instant::now());
    assert!(result.is_ok());
    assert_eq!(attempts, 2);
}

#[test]
fn permanent_offline_and_long_waits_stop_at_once() {
    let long_wait = DownloadError::RateLimited {
        status: 429,
        retry_after: MAX_RETRY_WAIT + Duration::from_secs(1),
    };
    for error in [
        DownloadError::Permanent("http status 400".into()),
        DownloadError::NotFound,
        DownloadError::Transient("offline".into()),
        long_wait,
    ] {
        let (result, attempts, sleeps) = run(RetryPolicy::default(), vec![error], Instant::now());
        assert!(result.is_err());
        assert_eq!((attempts, sleeps.len()), (1, 0));
    }
}
