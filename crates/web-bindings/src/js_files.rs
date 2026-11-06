//! wasm32 only: the content source the web worker fills before calling into Rust.
//! Large files are read on demand through a synchronous JS `read_at(offset, len) →
//! Uint8Array` (a Worker with `FileReaderSync` on a File System Access `File`);
//! small sheets (cue / m3u / gdi) can be handed over as bytes. Files live in a
//! thread-local table keyed by virtual path (`/web/<folder id>/<relative path>`),
//! looked up like Windows paths (`virtual_paths::lookup_key`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::js_file_reader::{self, JsFile};
use crate::virtual_paths::lookup_key;
use content_file_source::{with_source, ContentFile, ContentFileSource};

enum Entry {
    /// `read_at` callback number (`js_file_reader`), file length.
    Js {
        reader: usize,
        len: u64,
    },
    Bytes(Arc<[u8]>),
}

thread_local! {
    /// Lookup key → (path as registered, content).
    static FILES: RefCell<HashMap<String, (PathBuf, Entry)>> = RefCell::new(HashMap::new());
}

/// Forgets every registered file (call before registering the next game's files).
pub fn clear() {
    FILES.with(|files| files.borrow_mut().clear());
    js_file_reader::clear_readers();
}

/// Registers `path` read through `read_at`.
pub fn add_reader(path: &str, len: u64, read_at: js_sys::Function) {
    let reader = js_file_reader::push_reader(read_at);
    insert(path, Entry::Js { reader, len });
}

/// Registers `path` with its whole content.
pub fn add_bytes(path: &str, bytes: Vec<u8>) {
    insert(path, Entry::Bytes(bytes.into()));
}

fn insert(path: &str, entry: Entry) {
    let key = lookup_key(Path::new(path));
    FILES.with(|files| files.borrow_mut().insert(key, (path.into(), entry)));
}

/// Runs `f` with the registered files as the content source.
pub fn with_files<R>(f: impl FnOnce() -> R) -> R {
    with_source(Rc::new(RegisteredFiles), f)
}

struct RegisteredFiles;

fn not_found(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, path.display().to_string())
}

impl ContentFileSource for RegisteredFiles {
    fn open(&self, path: &Path) -> io::Result<ContentFile> {
        FILES.with(|files| {
            match files
                .borrow()
                .get(&lookup_key(path))
                .map(|(_, entry)| entry)
            {
                Some(Entry::Js { reader, len }) => {
                    Ok(Box::new(JsFile::new(*reader, *len)) as ContentFile)
                }
                Some(Entry::Bytes(bytes)) => {
                    Ok(Box::new(Cursor::new(SharedBytes(bytes.clone()))) as ContentFile)
                }
                None => Err(not_found(path)),
            }
        })
    }

    fn len(&self, path: &Path) -> io::Result<u64> {
        FILES.with(|files| {
            match files
                .borrow()
                .get(&lookup_key(path))
                .map(|(_, entry)| entry)
            {
                Some(Entry::Js { len, .. }) => Ok(*len),
                Some(Entry::Bytes(bytes)) => Ok(bytes.len() as u64),
                None => Err(not_found(path)),
            }
        })
    }

    fn is_file(&self, path: &Path) -> bool {
        FILES.with(|files| files.borrow().contains_key(&lookup_key(path)))
    }

    /// The path as registered (sheets may name it in another case or via `..`).
    fn canonical(&self, path: &Path) -> PathBuf {
        let key = lookup_key(path);
        FILES
            .with(|files| {
                files
                    .borrow()
                    .get(&key)
                    .map(|(registered, _)| registered.clone())
            })
            .unwrap_or_else(|| PathBuf::from(key))
    }
}

struct SharedBytes(Arc<[u8]>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
