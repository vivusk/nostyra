//! wasm32 only: library helpers beyond one file's identity — the metadat index of a
//! system (on `WebDat`), the stored-game metadata lookup ("Reset to database
//! values") and multi-file disc grouping. JSON shapes: `library_json`.

use wasm_bindgen::prelude::*;

use crate::identity_json;
use crate::js_files;
use crate::library_json::{self, GroupFile};
use crate::wasm_exports::{js_err, to_json, WebDat};

/// Scan verdict of a registered file (JSON `ScanVerdict`: `{kind:"ok",system}` /
/// `{kind:"skip"}` / `{kind:"problem",reason}`): the desktop scanner's policy.
#[wasm_bindgen]
pub fn classify_for_scan(path: &str) -> Result<String, JsError> {
    to_json(&js_files::with_files(|| {
        identity_json::classify_for_scan(std::path::Path::new(path))
    }))
}

/// Version stamped on metadata rows (the vendored metadat commit; user edits too).
#[wasm_bindgen]
pub fn metadata_source_version() -> String {
    rom_identity::METADATA_SOURCE_VERSION.to_string()
}

/// Metadat attribute folders (`developer`, `publisher`, `genre`, `releaseyear`,
/// `maxusers`): the order `WebDat.set_metadata` takes their texts in.
#[wasm_bindgen]
pub fn metadata_attribute_folders() -> Vec<String> {
    rom_identity::metadata_attribute_folders()
        .iter()
        .map(|folder| (*folder).to_string())
        .collect()
}

#[wasm_bindgen]
impl WebDat {
    /// Loads the system's metadat from the five attribute texts (folder order of
    /// `metadata_attribute_folders`; `undefined` = not fetched → partial index).
    pub fn set_metadata(
        &mut self,
        system: &str,
        developer: Option<String>,
        publisher: Option<String>,
        genre: Option<String>,
        releaseyear: Option<String>,
        maxusers: Option<String>,
    ) -> Result<(), JsError> {
        let texts = [developer, publisher, genre, releaseyear, maxusers];
        self.metadata = library_json::metadata_index(system, texts).map_err(js_err)?;
        Ok(())
    }

    /// Index metadata of a stored game (JSON `MetadataView`, or `null`).
    pub fn metadata_for_game(
        &self,
        dat_name: Option<String>,
        crc32: &str,
        serial: Option<String>,
    ) -> Result<String, JsError> {
        let view = library_json::stored_metadata(
            &self.index,
            &self.metadata,
            dat_name.as_deref(),
            crc32,
            serial.as_deref(),
        );
        to_json(&view)
    }
}

/// Groups the multi-file disc games of one folder walk over the registered files.
/// `files_json` = `[{path, disc}]` (classified files), `failures_json` = `[path]`;
/// returns JSON `GroupingView` (indices into both lists).
#[wasm_bindgen]
pub fn group_disc_files(files_json: &str, failures_json: &str) -> Result<String, JsError> {
    let files: Vec<GroupFile> = serde_json::from_str(files_json).map_err(js_err)?;
    let failures: Vec<String> = serde_json::from_str(failures_json).map_err(js_err)?;
    to_json(&js_files::with_files(|| {
        library_json::group_files(&files, &failures)
    }))
}
