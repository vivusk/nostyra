//! Contract between the launcher and `nostyra-runner`: stdout JSON-line events,
//! process exit codes, the shared data root, pending play-session files and
//! save-state files (numbered slots and the `auto` resume point) and the log files
//! both write under `<data>/logs/`.
//! Both sides depend on this crate so the contract lives in one place.

#![deny(unsafe_code)]

mod data_root_location;
mod game_folder_key;
pub mod log_files;
mod rom_key;
mod runner_events;
mod save_state_files;
mod shared_data_paths;
mod state_compatibility;
mod state_slot_id;

pub use data_root_location::{
    data_location_file, data_root, read_data_location, write_data_location,
};
pub use game_folder_key::{
    content_id, content_id_of_name, find_game_entry, game_entry_name, resolve_game_entry,
    CONTENT_ID_LEN,
};
pub use rom_key::rom_key;
pub use runner_events::{decode_event_line, encode_event_line, LauncherCommand, RunnerEvent};
pub use save_state_files::{
    state_file, state_slot_file, StateSidecar, MAX_STATE_SLOT, SIDECAR_FILE_EXT, STATE_FILE_EXT,
    STATE_SLOT_FILE_EXTS, THUMBNAIL_FILE_EXT,
};
pub use shared_data_paths::{
    config_dir, default_data_root, game_config_path_for_game, game_config_system_dir,
    pending_session_file_name, pending_sessions_dir, portable_dir, saves_dir_for_game, saves_root,
    saves_system_dir, staged_content_dir, system_dir, PendingSessionRecord, GAME_CONFIG_EXT,
    PORTABLE_DATA_DIR, PORTABLE_MARKER_FILE, RTC_FILE_NAME, SRAM_FILE_NAME,
};
pub use state_compatibility::{state_incompatibility, StateIncompatibility};
pub use state_slot_id::{StateSlotId, AUTO_STATE_STEM};

/// Clean exit.
pub const EXIT_OK: i32 = 0;
/// Bad command-line arguments (clap's default code).
pub const EXIT_USAGE: i32 = 2;
/// Core library missing, not a libretro core, or wrong API version.
pub const EXIT_CORE_LOAD: i32 = 3;
/// ROM missing, out of size limits, or rejected by the core.
pub const EXIT_ROM_LOAD: i32 = 4;
/// Failure while running (a GPU error mid-game, audio, a core error mid-game). A
/// graphics API this PC lacks is [`EXIT_GRAPHICS`] instead.
pub const EXIT_RUNTIME: i32 = 5;
/// The core needs a GPU API this PC cannot provide (e.g. OpenGL 3.3 for the
/// Nintendo 64): the game never started; a driver update may fix it.
pub const EXIT_GRAPHICS: i32 = 6;

/// libretro API version the host implements (reported by `--probe-core`).
pub const LIBRETRO_API_VERSION: u32 = 1;
