//! Tauri commands, grouped by feature. Registered in `lib.rs`; mirrored by
//! `src/api/tauri-commands*.ts`.

pub mod achievements_commands;
pub mod achievements_game_view;
pub mod achievements_mode;
mod achievements_snapshot_rows;
pub mod artwork_auto_fetch_hook;
pub mod artwork_commands;
pub mod artwork_fetch_job;
mod artwork_job_accounting;
mod artwork_work_list;
pub mod bios_commands;
mod bios_import;
mod blocking;
pub mod core_commands;
mod core_resolution;
pub mod data_folder_commands;
pub mod dev_seed_commands;
mod game_data_target;
pub mod game_detail_commands;
pub mod gamepad_capture_commands;
pub mod gameplay_config_commands;
pub mod home_commands;
mod input_binding_assign;
pub mod input_catalog_commands;
mod input_config_view;
pub mod library_commands;
pub mod log_commands;
pub mod memory_card_commands;
mod memory_card_import;
pub mod missing_game_removal_commands;
pub mod play_commands;
pub mod resume_commands;
mod resume_point;
pub mod save_state_commands;
pub mod scan_commands;
mod state_load_rule;
pub mod system_catalog_commands;
