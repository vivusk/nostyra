//! wasm32 only: naming (entries, web state stems, sidecars) and config-layer
//! exports. Same marshalling rules as `wasm_exports`.

use wasm_bindgen::prelude::*;

use crate::wasm_exports::{js_err, to_json};
use crate::{config_json, naming_json, web_play_json};

/// `<rom key> [ID]<ext>` for a new save folder / game layer.
#[wasm_bindgen]
pub fn new_entry_name(rom_name: &str, sha1: &str, ext: &str) -> Option<String> {
    naming_json::new_entry_name(rom_name, sha1, ext)
}

/// The existing entry of `sha1` among one folder's `names`.
#[wasm_bindgen]
pub fn find_entry_name(names: Vec<String>, sha1: &str, ext: &str) -> Option<String> {
    naming_json::find_entry_name(&names, sha1, ext)
}

/// `web-slot<n>` / `web-auto` (`slot` < 0).
#[wasm_bindgen]
pub fn web_state_stem(slot: i32) -> Option<String> {
    naming_json::state_stem(slot)
}

/// Sidecar JSON of a web save state.
#[wasm_bindgen]
pub fn web_state_sidecar(
    ejs_core: &str,
    created_at: f64,
    sram_crc32: Option<u32>,
    rom_patch_crc32: Option<u32>,
    state: &[u8],
) -> String {
    naming_json::web_state_sidecar(
        ejs_core,
        created_at as u64,
        sram_crc32,
        rom_patch_crc32,
        state,
    )
}

/// Why a web state (sidecar JSON) cannot load into `ejs_core`; `undefined` = it can.
#[wasm_bindgen]
pub fn web_state_refusal(sidecar_json: &str, ejs_core: &str) -> Option<String> {
    web_play_json::web_state_refusal(sidecar_json, ejs_core)
}

/// Per-game file names shared with the desktop (JSON `{sram, webSram, rtc}`).
#[wasm_bindgen]
pub fn data_file_names() -> Result<String, JsError> {
    to_json(&web_play_json::DATA_FILE_NAMES)
}

/// File name of a web pending play session (`<started ms>-web.json`).
#[wasm_bindgen]
pub fn web_pending_session_file_name(started_unix_ms: f64) -> String {
    web_play_json::web_pending_session_file_name(started_unix_ms as u64)
}

/// Pending play-session JSON for the desktop import (`sessions/pending/`).
#[wasm_bindgen]
pub fn web_pending_session_json(
    rom_path: &str,
    started_at: f64,
    play_seconds: f64,
    content_sha1: &str,
) -> String {
    web_play_json::web_pending_session_json(
        rom_path,
        started_at as i64,
        play_seconds as u64,
        content_sha1,
    )
}

#[wasm_bindgen]
pub fn crc32(bytes: &[u8]) -> u32 {
    naming_json::crc32(bytes)
}

/// Resolved known values + warnings (JSON) of layer texts, lowest first.
#[wasm_bindgen]
pub fn config_resolve(
    system: Option<String>,
    global: Option<String>,
    system_layer: Option<String>,
    core: Option<String>,
    game: Option<String>,
) -> Result<String, JsError> {
    let view = config_json::resolve(system.as_deref(), &[global, system_layer, core, game]);
    to_json(&view)
}

/// Known values one layer text sets (JSON object, dotted keys).
#[wasm_bindgen]
pub fn config_layer_values(text: Option<String>) -> Result<String, JsError> {
    to_json(&config_json::layer_values(text.as_deref()).map_err(js_err)?)
}

/// New layer text after `edits_json` (`[{key, value|null}]`); all or nothing.
#[wasm_bindgen]
pub fn config_set_values(text: Option<String>, edits_json: &str) -> Result<String, JsError> {
    let edits: Vec<config_json::KeyEdit> = serde_json::from_str(edits_json).map_err(js_err)?;
    config_json::set_values(text.as_deref(), &edits).map_err(js_err)
}
