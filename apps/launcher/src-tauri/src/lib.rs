//! Nostyra launcher backend (Tauri 2): thin command layer over the workspace crates
//! (storage-db, game-library, artwork-cache, core-manager, runner-session). Long work
//! runs on worker threads and reports through events (`app_events`).

#![deny(unsafe_code)]

mod achievements_journal_flush;
mod app_events;
mod app_state;
mod asset_scope_dirs;
mod bug_report_system_info;
mod bug_report_zip;
mod cloud_app_host;
mod cloud_change_detection;
mod cloud_close_wait;
mod cloud_game_result;
mod cloud_game_sync;
mod cloud_google_drive_account;
mod cloud_job_guard;
mod cloud_library_pass;
mod cloud_oauth_account_cleanup;
mod cloud_provider_factory;
mod cloud_sync_jobs;
mod cloud_sync_state;
mod cloud_unit_guard;
mod command_error;
mod command_handlers;
mod commands;
mod dat_file_path;
mod data_folder_move;
mod gamepad_capture_filter;
mod gamepad_capture_state;
mod gamepad_capture_thread;
mod legacy_game_files_migration;
mod log_secret_scrub;
mod metadata_self_heal;
mod relinked_game_files;
pub mod runner_binary_path;
mod shader_presets_dir;
mod update_check_settings;
mod update_check_status;
mod update_release_feed;
mod update_release_rules;
mod update_release_types;

#[cfg(test)]
mod cloud_play_guard_tests;
#[cfg(test)]
mod cloud_sync_flow_tests;
#[cfg(test)]
mod cloud_test_harness;
#[cfg(test)]
mod cloud_unit_guard_tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use ipc_protocol::log_files::{init_logging, LogFileKind, LogFileRequest};
use tauri::webview::PageLoadEvent;
use tauri::Manager;

use crate::app_state::AppState;
use crate::commands::achievements_commands;
use crate::gamepad_capture_state::GamepadCapture;

/// Starts the launcher window and blocks until the app exits.
pub fn run() -> anyhow::Result<()> {
    let started = Instant::now();
    let first_load_logged = AtomicBool::new(false);
    start_launcher_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GamepadCapture::default())
        .manage(achievements_commands::RaAccountCache::default())
        .setup(|app| {
            let state = AppState::init(app.handle())?;
            // Before any Play: old `<rom key>` save folders get their content id.
            legacy_game_files_migration::migrate_legacy_game_files_at_startup(
                &state.paths.data_root,
                &state.db(),
            );
            asset_scope_dirs::allow_artwork_dir(app.handle(), &state.paths.artwork_dir)?;
            asset_scope_dirs::allow_saves_dir(app.handle(), &state.paths.data_root)?;
            asset_scope_dirs::allow_badge_cache_dir(app.handle(), &state.paths.data_root)?;
            app.manage(state);
            metadata_self_heal::spawn_catalog_warmup(app.handle());
            achievements_journal_flush::spawn_startup_flush(app.handle());
            cloud_app_host::setup(app.handle());
            Ok(())
        })
        // Cold-start metric (PDR G1: < 1.5 s): process start → first page loaded.
        .on_page_load(move |_webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && !first_load_logged.swap(true, Ordering::Relaxed)
            {
                let ms = started.elapsed().as_millis();
                tracing::info!(startup_ms = ms, "launcher window ready");
            }
        })
        .invoke_handler(command_handlers::invoke_handler())
        .run(tauri::generate_context!())?;
    Ok(())
}

/// stderr plus `<data>/logs/launcher-YYYYMMDD-HHMMSS.log` (same data root `AppState`
/// opens; old logs are cleaned up first). A data-folder change restarts the app, so the
/// new folder gets a fresh log.
fn start_launcher_logging() {
    let data_root = ipc_protocol::data_root();
    let request = data_root.as_deref().map(|data_root| LogFileRequest {
        data_root,
        kind: LogFileKind::Launcher,
        label: None,
    });
    if let Some(path) = init_logging(request) {
        tracing::info!(log_file = %path.display(), "launcher log file");
    }
}
