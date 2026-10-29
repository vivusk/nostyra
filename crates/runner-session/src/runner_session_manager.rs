//! Spawns one runner game session and turns its output and exit into
//! [`SessionUpdate`]s on a channel. Threads: stdout reader, stderr reader, and a
//! coordinator that owns the child, watches the launch timeout and reports the end.

use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ipc_protocol::RunnerEvent;

use crate::runner_exit_classification::classify_exit;
use crate::runner_launch::RunnerLaunch;
use crate::runner_output_readers::{spawn_stderr_reader, spawn_stdout_reader, StderrTail};
use crate::runner_process_command::runner_session_command;
use crate::session_update::{AchievementUpdate, SessionError, SessionOutcome, SessionUpdate};

/// A running session; drop the receiver to stop listening (the game keeps running).
pub struct RunnerSession {
    pub pid: u32,
    pub updates: Receiver<SessionUpdate>,
}

/// Starts the runner with [`crate::runner_args`], the launch env and no console window.
pub fn spawn_runner_session(
    launch: &RunnerLaunch,
    launch_timeout: Duration,
) -> std::io::Result<RunnerSession> {
    let mut child = runner_session_command(launch).spawn()?;
    let pid = child.id();
    let (event_tx, event_rx) = mpsc::channel();
    let tail = StderrTail::default();
    // take() always succeeds: both pipes were configured as piped above.
    let stdout = child.stdout.take().ok_or_else(missing_pipe)?;
    let stderr = child.stderr.take().ok_or_else(missing_pipe)?;
    spawn_stdout_reader(stdout, event_tx)?;
    let stderr_thread = spawn_stderr_reader(stderr, tail.clone())?;
    let (update_tx, update_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("runner-session".into())
        .spawn(move || {
            let coordinator = Coordinator {
                child,
                event_rx,
                update_tx,
                tail,
                stderr_thread: Some(stderr_thread),
            };
            coordinator.run(launch_timeout);
        })?;
    Ok(RunnerSession {
        pid,
        updates: update_rx,
    })
}

fn missing_pipe() -> std::io::Error {
    std::io::Error::other("runner pipe not available")
}

struct Coordinator {
    child: Child,
    event_rx: Receiver<RunnerEvent>,
    update_tx: Sender<SessionUpdate>,
    tail: StderrTail,
    stderr_thread: Option<JoinHandle<()>>,
}

impl Coordinator {
    fn run(mut self, launch_timeout: Duration) {
        let deadline = Instant::now() + launch_timeout;
        let mut started: Option<(Instant, i64)> = None;
        let mut timed_out = false;
        let mut error_message = None;
        loop {
            let event = if started.is_none() && !timed_out {
                self.event_rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            } else {
                self.event_rx
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected)
            };
            match event {
                Ok(RunnerEvent::Started { core, rom }) => {
                    started.get_or_insert((Instant::now(), unix_now()));
                    self.send(SessionUpdate::Started { core, rom });
                }
                Ok(RunnerEvent::Error { message }) => error_message = Some(message),
                // Stopped: play time comes from the pending file.
                Ok(other) => {
                    if let Some(update) = AchievementUpdate::from_event(other) {
                        self.send(SessionUpdate::Achievements(update));
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    self.send(SessionUpdate::LaunchTimedOut);
                }
                // stdout closed: the runner is exiting.
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        let exit_code = self.child.wait().ok().and_then(|status| status.code());
        if let Some(thread) = self.stderr_thread.take() {
            let _ = thread.join(); // complete tail before reading it
        }
        let error = classify_exit(exit_code).map(|kind| SessionError {
            code: exit_code,
            kind,
            detail: error_message.unwrap_or_else(|| self.tail.joined()),
        });
        self.send(SessionUpdate::Finished(SessionOutcome {
            exit_code,
            started_at: started.map(|(_, unix)| unix),
            measured_seconds: started.map_or(0, |(at, _)| at.elapsed().as_secs()),
            error,
        }));
    }

    fn send(&self, update: SessionUpdate) {
        // Nobody listening (launcher shutting down) is fine.
        let _ = self.update_tx.send(update);
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}
