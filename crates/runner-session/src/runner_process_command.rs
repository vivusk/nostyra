//! Builds runner `Command`s: explicit args (no shell), piped output, and no console
//! window on Windows (the runner is a console-subsystem exe); game sessions also get
//! the launch's extra env.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::runner_launch::{runner_args, RunnerLaunch};

/// `CREATE_NO_WINDOW` process creation flag.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A runner command with stdin closed and stdout/stderr piped.
pub fn runner_command(runner: &Path) -> Command {
    let mut command = Command::new(runner);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// The game-session command: [`runner_args`] plus the launch's extra env (the RA
/// login; set on the child only, never on the launcher process).
pub fn runner_session_command(launch: &RunnerLaunch) -> Command {
    let mut command = runner_command(&launch.runner);
    command.args(runner_args(launch));
    command.envs(launch.env.iter().map(|(name, value)| (name, value)));
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn session_command_applies_the_launch_env() {
        let launch = RunnerLaunch {
            runner: "runner".into(),
            core: "core".into(),
            rom: "rom".into(),
            system: "snes".into(),
            data_dir: "data".into(),
            extra_args: Vec::new(),
            env: vec![("NOSTYRA_RA_USER".into(), "Tester".into())],
        };
        let command = runner_session_command(&launch);
        let envs: Vec<_> = command.get_envs().collect();
        assert_eq!(
            envs,
            [(OsStr::new("NOSTYRA_RA_USER"), Some(OsStr::new("Tester")))]
        );
        assert_eq!(command.get_args().count(), runner_args(&launch).len());
    }
}
