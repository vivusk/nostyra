//! One door for every content-file read of the identity code (system-catalog
//! classifier, disc-image readers, rom-identity hashers): by default the OS file
//! system, exactly as `std::fs` (desktop behaviour unchanged). The web edition has no
//! file paths: around a synchronous wasm call it installs a [`ContentFileSource`] on
//! the current thread ([`with_source`]) that serves virtual paths from File System
//! Access reads (a Worker with `FileReaderSync`), so the same path-based code — cue
//! sheets naming their bin files, m3u playlists, disc grouping — runs unchanged.

#![deny(unsafe_code)]

mod memory_files;

use std::cell::RefCell;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use memory_files::MemoryFiles;

/// A readable, seekable content file.
pub trait ContentRead: Read + Seek + Send {}
impl<T: Read + Seek + Send> ContentRead for T {}

/// An open content file.
pub type ContentFile = Box<dyn ContentRead>;

/// Where content files come from.
pub trait ContentFileSource {
    /// Opens `path` for reading (`NotFound` when it does not exist).
    fn open(&self, path: &Path) -> io::Result<ContentFile>;
    /// Length in bytes (`std::fs::metadata(path)?.len()` on the OS).
    fn len(&self, path: &Path) -> io::Result<u64>;
    /// `true` when `path` is an existing file.
    fn is_file(&self, path: &Path) -> bool;
    /// The form two paths of the same file compare equal in (disc grouping:
    /// `a/../B.BIN` vs `b.bin`); the path itself when the source cannot tell.
    fn canonical(&self, path: &Path) -> PathBuf {
        path.to_path_buf()
    }
}

/// The OS file system (the default source).
#[derive(Debug, Clone, Copy, Default)]
pub struct OsFiles;

impl ContentFileSource for OsFiles {
    fn open(&self, path: &Path) -> io::Result<ContentFile> {
        Ok(Box::new(std::fs::File::open(path)?))
    }

    fn len(&self, path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.len())
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

thread_local! {
    static SOURCE: RefCell<Option<Rc<dyn ContentFileSource>>> = const { RefCell::new(None) };
}

fn current() -> Option<Rc<dyn ContentFileSource>> {
    SOURCE.with(|cell| cell.borrow().clone())
}

/// Opens `path` from the current thread's source.
pub fn open(path: &Path) -> io::Result<ContentFile> {
    match current() {
        Some(source) => source.open(path),
        None => OsFiles.open(path),
    }
}

/// Length of `path` from the current thread's source.
pub fn len(path: &Path) -> io::Result<u64> {
    match current() {
        Some(source) => source.len(path),
        None => OsFiles.len(path),
    }
}

/// Whether `path` is an existing file in the current thread's source.
pub fn is_file(path: &Path) -> bool {
    match current() {
        Some(source) => source.is_file(path),
        None => OsFiles.is_file(path),
    }
}

/// Comparable form of `path` in the current thread's source (see
/// [`ContentFileSource::canonical`]).
pub fn canonical(path: &Path) -> PathBuf {
    match current() {
        Some(source) => source.canonical(path),
        None => OsFiles.canonical(path),
    }
}

/// Whole file as UTF-8 text (`std::fs::read_to_string`).
pub fn read_to_string(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    open(path)?.read_to_string(&mut text)?;
    Ok(text)
}

/// Total length of an open file (seeks to the end and back).
pub fn stream_len(file: &mut dyn ContentRead) -> io::Result<u64> {
    let pos = file.stream_position()?;
    let end = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(pos))?;
    Ok(end)
}

/// Runs `f` with `source` serving every read on this thread, then restores the
/// previous source (also when `f` panics).
pub fn with_source<R>(source: Rc<dyn ContentFileSource>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<dyn ContentFileSource>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            SOURCE.with(|cell| *cell.borrow_mut() = previous);
        }
    }
    let previous = SOURCE.with(|cell| cell.borrow_mut().replace(source));
    let _restore = Restore(previous);
    f()
}

#[cfg(test)]
#[path = "content_file_source_tests.rs"]
mod tests;
