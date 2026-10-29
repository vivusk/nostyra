//! `--probe-core <path>`: loads the library without a game and prints its identity
//! as a `CoreInfo` event, so the launcher can validate a download out of process.

use std::path::Path;
use std::process::ExitCode;

use ipc_protocol::{RunnerEvent, LIBRETRO_API_VERSION};

use crate::ipc_event_writer::emit_event;
use crate::runner_exit_codes::EXIT_CORE_LOAD;

pub fn run(core: &Path) -> ExitCode {
    match libretro_host::probe_core(core) {
        Ok(info) => {
            emit_event(&RunnerEvent::CoreInfo {
                name: info.name,
                version: info.version,
                api_version: LIBRETRO_API_VERSION,
            });
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!(core = %core.display(), "not a usable libretro core: {err}");
            emit_event(&RunnerEvent::Error {
                message: format!("not a usable libretro core: {err}"),
            });
            ExitCode::from(EXIT_CORE_LOAD)
        }
    }
}
