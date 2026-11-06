//! wasm32 only: the functions TypeScript calls (from Web Workers; all synchronous).
//! Pure marshalling over `catalog_json`, `identity_json`, `config_json`,
//! `naming_json`; structured results cross as JSON text.

use std::fmt::Display;
use std::path::Path;

use rom_identity::{DatIndex, MetadataIndex};
use wasm_bindgen::prelude::*;

use crate::{catalog_json, identity_json, js_files};

pub(crate) fn js_err(err: impl Display) -> JsError {
    JsError::new(&err.to_string())
}

pub(crate) fn to_json(value: &impl serde::Serialize) -> Result<String, JsError> {
    serde_json::to_string(value).map_err(js_err)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Panics reach the browser console instead of a bare "unreachable".
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&info.to_string())));
}

/// Every registry system (JSON array, see `catalog_json::WebSystemView`).
#[wasm_bindgen]
pub fn system_catalog_json() -> Result<String, JsError> {
    to_json(&catalog_json::web_systems())
}

/// Forgets the registered content files.
#[wasm_bindgen]
pub fn files_clear() {
    js_files::clear();
}

/// Registers a content file read on demand through `read_at(offset, len)`.
#[wasm_bindgen]
pub fn files_add(path: &str, len: f64, read_at: js_sys::Function) {
    js_files::add_reader(path, len as u64, read_at);
}

/// Registers a small content file (cue / m3u / gdi text) by its bytes.
#[wasm_bindgen]
pub fn files_add_bytes(path: &str, bytes: Vec<u8>) {
    js_files::add_bytes(path, bytes);
}

/// System id of a registered file; the error message is the scan-report reason.
#[wasm_bindgen]
pub fn classify(path: &str) -> Result<String, JsError> {
    js_files::with_files(|| identity_json::classify(Path::new(path)))
        .map(str::to_owned)
        .map_err(js_err)
}

/// A system's DAT index (bundled DAT text fetched by the worker) and, once
/// `set_metadata` ran, its metadat index (`wasm_library_exports`).
#[wasm_bindgen]
pub struct WebDat {
    pub(crate) index: DatIndex,
    pub(crate) metadata: MetadataIndex,
}

#[wasm_bindgen]
impl WebDat {
    #[wasm_bindgen(constructor)]
    pub fn new(system: &str, text: &str) -> Result<WebDat, JsError> {
        let index = identity_json::load_dat(system, text).map_err(js_err)?;
        Ok(Self {
            index,
            metadata: MetadataIndex::empty(),
        })
    }

    /// No DAT: every game needs review.
    pub fn empty() -> WebDat {
        Self {
            index: DatIndex::empty(),
            metadata: MetadataIndex::empty(),
        }
    }

    pub fn entries(&self) -> usize {
        self.index.len()
    }
}

/// Identity + DAT facts + metadata of a registered file as `system` (JSON
/// `IdentifiedGame`).
#[wasm_bindgen]
pub fn identify(path: &str, system: &str, dat: &WebDat) -> Result<String, JsError> {
    let game = js_files::with_files(|| {
        identity_json::identify(Path::new(path), system, &dat.index, &dat.metadata)
    })
    .map_err(js_err)?;
    to_json(&game)
}

/// Scan verdict of a registered `.zip` among `allowed_systems` (JSON `ScanVerdict`:
/// `{kind:"ok",system}` / `{kind:"skip"}` / `{kind:"problem",reason}`).
#[wasm_bindgen]
pub fn zip_single_rom(path: &str, allowed_systems: Vec<String>) -> Result<String, JsError> {
    let verdict =
        js_files::with_files(|| identity_json::zip_single_rom(Path::new(path), &allowed_systems));
    to_json(&verdict)
}

/// The ROM inside a registered single-ROM `.zip`.
#[wasm_bindgen]
pub struct ZipRom {
    name: String,
    system: String,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl ZipRom {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn system(&self) -> String {
        self.system.clone()
    }

    /// Moves the ROM bytes out (call once).
    pub fn take_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}

#[wasm_bindgen]
pub fn read_zip_rom(path: &str) -> Result<ZipRom, JsError> {
    let (name, system, bytes) =
        js_files::with_files(|| identity_json::read_zip_rom(Path::new(path))).map_err(js_err)?;
    Ok(ZipRom {
        name,
        system: system.to_owned(),
        bytes,
    })
}

/// Applies an IPS / UPS / BPS soft patch.
#[wasm_bindgen]
pub fn apply_patch(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, JsError> {
    identity_json::apply_patch(rom, patch).map_err(js_err)
}
