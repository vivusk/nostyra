//! Micro-benchmark (ignored by default): serialize + unserialize cost on Snes9x with
//! the run-ahead path (`serialize_into`, reused buffer). Own test binary, so its core
//! session never collides with `headless_snes9x_session` (one core per process).
//!
//! `cargo test -p libretro-host --release --test serialize_benchmark -- --ignored --nocapture`

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::time::{Duration, Instant};

use common::{core_path, load, Sinks};
use libretro_host::SavestateContext;

const ITERATIONS: u32 = 1000;

#[test]
#[ignore = "benchmark; run with --release -- --ignored --nocapture"]
fn snes9x_serialize_unserialize_cost() {
    let mut session = load(&core_path()).expect("load core + test ROM");
    Sinks::new().run(&mut session, 60);
    let ctx = SavestateContext::RunaheadSameInstance;
    let mut buf = Vec::new();
    let (mut serialize, mut unserialize) = (Duration::ZERO, Duration::ZERO);
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        session.serialize_into(&mut buf, ctx).unwrap();
        let mid = Instant::now();
        session.unserialize_from(&buf, ctx).unwrap();
        serialize += mid - started;
        unserialize += mid.elapsed();
    }
    let avg_us = |total: Duration| total.as_secs_f64() * 1e6 / f64::from(ITERATIONS);
    eprintln!(
        "state {} B: serialize avg {:.1} µs, unserialize avg {:.1} µs, total {:.1} µs",
        buf.len(),
        avg_us(serialize),
        avg_us(unserialize),
        avg_us(serialize + unserialize)
    );
}
