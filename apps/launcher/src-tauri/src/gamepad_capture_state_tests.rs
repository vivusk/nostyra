//! Capture exclusivity and the Play race, with real SDL and no pad needed.
//! (With a pad attached, do not touch it while these run.)

use std::time::Instant;

use super::*;

const IDLE: fn() -> bool = || false;

/// SDL allows one init thread at a time per process: these tests take turns.
static SDL_TURN: Mutex<()> = Mutex::new(());

/// Starts a 10 s capture on another thread and waits until it is running.
fn start_long_capture(
    capture: &Arc<GamepadCapture>,
) -> std::thread::JoinHandle<CommandResult<Option<CapturedInput>>> {
    let worker = {
        let capture = capture.clone();
        std::thread::spawn(move || capture.capture(Duration::from_secs(10), IDLE))
    };
    while !capture.is_running() {
        std::thread::yield_now();
    }
    worker
}

#[test]
fn idle_capture_times_out_cancels_and_is_exclusive() {
    let _turn = lock(&SDL_TURN);
    let capture = Arc::new(GamepadCapture::default());
    let started = Instant::now();
    assert!(matches!(
        capture.capture(Duration::from_millis(200), IDLE),
        Ok(None)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));

    let worker = start_long_capture(&capture);
    let second = capture.capture(Duration::from_millis(10), IDLE);
    assert_eq!(second.unwrap_err().kind, "busy");
    let cancelled = Instant::now();
    capture.cancel();
    assert!(matches!(worker.join().unwrap(), Ok(None)));
    assert!(cancelled.elapsed() < Duration::from_secs(2));
    assert_eq!(capture.label_style(), LabelStyle::Xbox);
}

#[test]
fn play_block_waits_for_a_running_capture_and_refuses_new_ones() {
    let _turn = lock(&SDL_TURN);
    let capture = Arc::new(GamepadCapture::default());
    let worker = start_long_capture(&capture);
    let block = capture.block_for_play().unwrap();
    // Returned only after the capture thread finished (SDL dropped).
    assert!(!capture.is_running());
    assert!(matches!(worker.join().unwrap(), Ok(None)));
    let refused = capture.capture(Duration::from_millis(10), IDLE);
    assert_eq!(refused.unwrap_err().kind, "busy");
    drop(block);
    // Session registered by then: the re-check under the lock refuses too.
    let refused = capture.capture(Duration::from_millis(10), || true);
    assert_eq!(refused.unwrap_err().kind, "busy");
    // Game over: captures work again.
    assert!(matches!(
        capture.capture(Duration::from_millis(10), IDLE),
        Ok(None)
    ));
}
