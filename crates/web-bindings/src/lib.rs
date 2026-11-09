//! The web edition's (nostyra.com/play) WebAssembly module: the same Rust logic the
//! desktop runs — system registry, content classification and identity, DAT
//! matching, single-ROM ZIPs, soft patches, entry naming, config layers, controls — behind a
//! thin wasm-bindgen surface (`wasm_exports`, wasm32 only). The marshalling-free
//! parts are plain modules, tested natively with the workspace.
//!
//! Content files are never read by path from the OS here: the web worker registers
//! them (`files_add`: a synchronous `read_at` over a File System Access `File`) under
//! virtual paths, and every call runs with them as the content source.
//!
//! Built by `scripts/build-web-wasm.ps1` / `.sh` (wasm-pack) into
//! `apps/launcher/src/backend/web/wasm-pkg/`.

#![deny(unsafe_code)]

pub mod catalog_json;
pub mod config_json;
pub mod controls_json;
pub mod dat_pending_json;
pub mod identity_json;
pub mod library_json;
pub mod naming_json;
pub mod virtual_paths;
pub mod web_play_json;

mod identity_parity;

#[cfg(target_arch = "wasm32")]
mod js_file_reader;
#[cfg(target_arch = "wasm32")]
mod js_files;
#[cfg(target_arch = "wasm32")]
mod wasm_controls_exports;
#[cfg(target_arch = "wasm32")]
mod wasm_dat_pending_exports;
#[cfg(target_arch = "wasm32")]
mod wasm_exports;
#[cfg(target_arch = "wasm32")]
mod wasm_library_exports;
#[cfg(target_arch = "wasm32")]
mod wasm_naming_config_exports;
