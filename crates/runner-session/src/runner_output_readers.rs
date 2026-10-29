//! Reader threads for the runner's pipes. Both drain their pipe to EOF so the
//! runner never blocks on a full pipe buffer.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ipc_protocol::{decode_event_line, RunnerEvent};

/// stderr lines kept for error details.
const STDERR_TAIL_LINES: usize = 20;

/// Last lines of runner stderr (its logs), shared with the coordinator thread.
#[derive(Debug, Default, Clone)]
pub struct StderrTail(Arc<Mutex<VecDeque<String>>>);

impl StderrTail {
    fn push(&self, line: String) {
        let mut lines = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if lines.len() == STDERR_TAIL_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// The kept lines joined with newlines.
    pub fn joined(&self) -> String {
        let lines = self.0.lock().unwrap_or_else(|p| p.into_inner());
        lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// Decodes stdout into events; core noise is skipped. Ends at EOF (runner exit).
pub fn spawn_stdout_reader(
    stdout: impl Read + Send + 'static,
    events: Sender<RunnerEvent>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("runner-stdout".into())
        .spawn(move || {
            for_each_line(stdout, |line| {
                if let Some(event) = decode_event_line(line) {
                    // Receiver gone = coordinator finished; keep draining anyway.
                    let _ = events.send(event);
                }
            })
        })
}

/// Collects stderr into `tail` (and debug logs).
pub fn spawn_stderr_reader(
    stderr: impl Read + Send + 'static,
    tail: StderrTail,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("runner-stderr".into())
        .spawn(move || {
            for_each_line(stderr, |line| {
                tracing::debug!(target: "nostyra_runner", "{line}");
                tail.push(line.to_string());
            })
        })
}

/// Lossy UTF-8 lines (a core may print anything); stops at EOF or read error.
fn for_each_line(pipe: impl Read, mut on_line: impl FnMut(&str)) {
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => on_line(String::from_utf8_lossy(&buf).trim_end()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdout_reader_decodes_events_between_noise() {
        let output = b"Map_LoROMMap\n{\"type\":\"Started\",\"core\":\"Snes9x\",\"rom\":\"a.sfc\"}\n\xFFjunk{\"type\":\"Stopped\",\"play_seconds\":3}\n".to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_stdout_reader(std::io::Cursor::new(output), tx)
            .unwrap()
            .join()
            .unwrap();
        let events: Vec<_> = rx.iter().collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1], RunnerEvent::Stopped { play_seconds: 3 });
    }

    #[test]
    fn stderr_tail_keeps_last_lines() {
        let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let tail = StderrTail::default();
        spawn_stderr_reader(std::io::Cursor::new(text.into_bytes()), tail.clone())
            .unwrap()
            .join()
            .unwrap();
        let joined = tail.joined();
        assert!(joined.starts_with("line 10\n") && joined.ends_with("line 29"));
    }
}
