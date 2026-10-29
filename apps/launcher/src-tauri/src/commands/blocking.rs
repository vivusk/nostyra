//! Runs blocking work (file system, DB waits, shell calls) off the main thread, which
//! is where plain `fn` Tauri commands execute.

use crate::command_error::{CommandError, CommandResult};

pub async fn run_blocking<T, F>(work: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| CommandError::invalid(format!("Background task failed: {err}")))?
}
