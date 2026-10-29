//! The only place with `unsafe` in this crate: hand-written rcheevos v12.5.0
//! declarations, `#[repr(C)]` mirrors (checked against C by `ffi_layout_tests`), the
//! rc_client trampolines and the safe wrappers the rest of the crate uses.

mod api_calls;
mod api_ffi;
mod api_replay_calls;
mod c_text;
mod client_ffi;
mod client_trampolines;
mod client_types;
mod event_conversion;
mod hash_cd_reader;
// Full v12.5.0 declarations; a few entry points stay unused.
#[allow(dead_code)]
mod hash_ffi;
mod hash_file_reader;
#[allow(dead_code)]
mod libretro_ffi;
mod option_rules;
mod raw_client;
mod raw_client_runtime;
mod raw_hash;
mod raw_memory_regions;
mod server_callback;
#[cfg(windows)]
mod windows_version;

#[cfg(test)]
mod ffi_layout_tests;

pub use api_calls::{error_text, login_request, parse_login_response};
pub use api_replay_calls::{
    award_request, parse_award_response, parse_submit_response, submit_request,
};
pub use option_rules::rc_setting_allowed;
pub use raw_client::RawClient;
pub use raw_hash::{generate_hash, HashReaders};
#[cfg(test)]
pub use raw_memory_regions::lookup_installed;
pub use raw_memory_regions::{CoreMemoryLookup, RawMemoryRegions};
pub use server_callback::PendingCallback;
#[cfg(windows)]
pub use windows_version::windows_version;

#[cfg(test)]
pub use server_callback::test_callbacks;
