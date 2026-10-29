//! Validates a core file out of process with `nostyra-runner --probe-core`, so a
//! crashing or hostile library never runs inside the launcher. Used as the
//! core-manager install probe (phase 06 wiring).

use std::path::Path;

use ipc_protocol::{decode_event_line, RunnerEvent};

use crate::runner_process_command::runner_command;

/// Identity reported by a probed core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedCore {
    pub name: String,
    pub version: String,
}

/// Runs the probe and waits for it. `Err` carries a readable reason.
pub fn probe_core_with_runner(runner: &Path, core: &Path) -> Result<ProbedCore, String> {
    let output = runner_command(runner)
        .arg("--probe-core")
        .arg(core)
        .output()
        .map_err(|err| format!("cannot run {}: {err}", runner.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut error = None;
    for event in stdout.lines().filter_map(decode_event_line) {
        match event {
            RunnerEvent::CoreInfo { name, version, .. } if output.status.success() => {
                return Ok(ProbedCore { name, version })
            }
            RunnerEvent::Error { message } => error = Some(message),
            _ => {}
        }
    }
    Err(error.unwrap_or_else(|| {
        format!(
            "core probe failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }))
}
