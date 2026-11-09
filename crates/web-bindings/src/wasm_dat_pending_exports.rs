//! wasm32 only: the DAT-pending exports of the web scan (`dat_pending_json`). The
//! DAT is **borrowed** (`&WebDat`), so the page's cached per-system handles
//! (`system-dat.ts`) stay valid for the next `identify`.

use std::path::Path;

use wasm_bindgen::prelude::*;

use crate::dat_pending_json;
use crate::js_files;
use crate::wasm_exports::{js_err, WebDat};

/// Ids of the systems whose DAT may still list a registered file the classifier
/// rejected (Atari `.bin`), in registry order.
#[wasm_bindgen]
pub fn dat_pending_candidates(path: &str) -> Vec<String> {
    js_files::with_files(|| dat_pending_json::candidates(Path::new(path)))
}

/// Whether `dat` (the DAT of `system_id`) lists the registered file at `path`.
#[wasm_bindgen]
pub fn dat_hit(path: &str, system_id: &str, dat: &WebDat) -> Result<bool, JsError> {
    js_files::with_files(|| dat_pending_json::dat_hit(Path::new(path), system_id, &dat.index))
        .map_err(js_err)
}

/// Problem text for a pending file no candidate DAT knows.
#[wasm_bindgen]
pub fn dat_pending_hint(reason: &str, candidate_ids: Vec<String>) -> String {
    dat_pending_json::hint(reason, &candidate_ids)
}
