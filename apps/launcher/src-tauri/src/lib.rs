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
mod command_error;
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use ipc_protocol::log_files::{init_logging, LogFileKind, LogFileRequest};
use tauri::webview::PageLoadEvent;
use tauri::Manager;

use crate::app_state::AppState;
use crate::commands::{
    achievements_commands, achievements_game_view, achievements_mode, artwork_commands,
    bios_commands, core_commands, data_folder_commands, dev_seed_commands, game_detail_commands,
    gamepad_capture_commands, gameplay_config_commands, home_commands, input_catalog_commands,
    library_commands, log_commands, memory_card_commands, missing_game_removal_commands,
    play_commands, resume_commands, save_state_commands, scan_commands, system_catalog_commands,
};
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
        .invoke_handler(tauri::generate_handler![
            library_commands::get_setup_state,
            library_commands::list_library_folders,
            library_commands::add_library_folder,
            library_commands::remove_library_folder,
            library_commands::list_games,
            library_commands::library_summary,
            system_catalog_commands::get_systems,
            system_catalog_commands::get_library_systems,
            missing_game_removal_commands::remove_missing_game,
            missing_game_removal_commands::remove_all_missing_games,
            scan_commands::start_scan,
            scan_commands::cancel_scan,
            scan_commands::get_scan_progress,
            core_commands::get_core_status,
            core_commands::download_core,
            play_commands::play_game,
            play_commands::get_active_session,
            game_detail_commands::get_game_detail,
            game_detail_commands::set_favorite,
            game_detail_commands::update_game_metadata,
            game_detail_commands::reset_game_metadata,
            game_detail_commands::list_genres,
            game_detail_commands::reveal_rom_in_folder,
            home_commands::get_home_data,
            artwork_commands::start_artwork_fetch,
            artwork_commands::cancel_artwork_fetch,
            artwork_commands::get_artwork_progress,
            artwork_commands::get_artwork_cache_stats,
            artwork_commands::clear_artwork_cache,
            artwork_commands::get_artwork_auto_fetch,
            artwork_commands::set_artwork_auto_fetch,
            gameplay_config_commands::get_gameplay_config,
            gameplay_config_commands::set_gameplay_config_value,
            save_state_commands::list_save_states,
            save_state_commands::delete_save_state,
            input_catalog_commands::get_input_catalog,
            input_catalog_commands::assign_binding,
            input_catalog_commands::reset_controls,
            gamepad_capture_commands::capture_gamepad_input,
            gamepad_capture_commands::cancel_gamepad_capture,
            gamepad_capture_commands::gamepad_label_style,
            resume_commands::get_resume_info,
            achievements_mode::achievements_capabilities,
            achievements_commands::ra_login,
            achievements_commands::ra_logout,
            achievements_commands::ra_account,
            achievements_game_view::game_achievements,
            bios_commands::get_bios_status,
            bios_commands::add_bios_file,
            bios_commands::open_system_folder,
            memory_card_commands::get_memory_cards,
            memory_card_commands::import_memory_card,
            data_folder_commands::check_data_folder,
            data_folder_commands::change_data_folder,
            dev_seed_commands::seed_fake_library,
            log_commands::open_logs_folder,
            log_commands::create_bug_report,
        ])
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
