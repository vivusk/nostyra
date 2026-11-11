//! Tests of `frame_pacer` (pacing, cap, speed scale, fps meter).

use super::*;

#[test]
fn runs_core_rate_on_fast_display() {
    // 144 Hz redraws for one second at 60 fps → ~60 frames.
    let mut pacer = FramePacer::new(60.0);
    let start = Instant::now();
    let total: u32 = (0..=144)
        .map(|i| pacer.frames_due(start + Duration::from_secs_f64(i as f64 / 144.0)))
        .sum();
    assert!((60..=61).contains(&total), "{total}");
}

#[test]
fn stall_is_capped() {
    let mut pacer = FramePacer::new(60.0);
    let start = Instant::now();
    pacer.frames_due(start);
    assert_eq!(
        pacer.frames_due(start + Duration::from_secs(5)),
        MAX_FRAMES_PER_TICK
    );
}

#[test]
fn reset_drops_the_backlog() {
    let mut pacer = FramePacer::new(60.0);
    let start = Instant::now();
    pacer.frames_due(start);
    pacer.reset();
    // After a long fast-forward the next tick runs one frame, not a catch-up burst.
    assert_eq!(pacer.frames_due(start + Duration::from_secs(5)), 1);
}

#[test]
fn fps_meter_reports_after_window() {
    let mut meter = FpsMeter::new();
    let start = Instant::now();
    assert_eq!(meter.record(start, 0), None);
    let fps = meter.record(start + FpsMeter::REPORT_EVERY, 300).unwrap();
    assert!((fps - 60.0).abs() < 0.01, "{fps}");
}

#[test]
fn bogus_fps_falls_back_to_60() {
    assert_eq!(frame_time(0.0), Duration::from_secs_f64(1.0 / 60.0));
}

#[test]
fn netplay_cap_and_speed_scale() {
    let start = Instant::now();
    let mut pacer = FramePacer::with_cap(60.0, 12);
    pacer.frames_due(start);
    let after = |ms: u64| start + Duration::from_millis(ms);
    assert_eq!(
        pacer.frames_due(after(201)),
        12,
        "200 ms backlog: 12, not 3"
    );
    let mut slow = FramePacer::with_cap(60.0, 12);
    slow.set_speed_scale(1.10);
    slow.frames_due(start);
    let due = slow.frames_due(after(200));
    assert!((10..=11).contains(&due), "{due}");
    slow.set_speed_scale(1.0);
    slow.frames_due(after(200));
    let due = slow.frames_due(after(400));
    assert!((12..=13).contains(&due), "{due}");
    slow.set_speed_scale(f64::NAN);
    assert_eq!(slow.frame_time, frame_time(60.0), "bogus scale = 1.0");
}
