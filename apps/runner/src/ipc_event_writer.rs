//! stdout output that never panics. `println!` panics when the reader is gone
//! (launcher closed mid-game → BrokenPipe); a lost status line must never change
//! the runner's exit code or interrupt the game.

use std::io::Write;

use ipc_protocol::{encode_event_line, RunnerEvent};

/// Writes `text` to stdout and flushes; write errors are logged and ignored.
pub fn write_stdout(text: &str) {
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        tracing::debug!(%err, "stdout closed; dropping runner output");
    }
}

/// Emits one launcher event (`"\n" + json + "\n"`, see ipc-protocol).
pub fn emit_event(event: &RunnerEvent) {
    write_stdout(&encode_event_line(event));
}
