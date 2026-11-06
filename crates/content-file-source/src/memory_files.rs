//! In-memory source: virtual paths → bytes. Tests use it to prove the identity code
//! runs with no OS files; the web edition uses it for small files (cue / m3u / gdi
//! text) next to its File System Access reader.

use std::collections::HashMap;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{ContentFile, ContentFileSource};

/// Files held in memory, keyed by their (virtual) path.
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: HashMap<PathBuf, Arc<[u8]>>,
}

impl MemoryFiles {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (or replaces) the file at `path`.
    pub fn insert(&mut self, path: impl Into<PathBuf>, bytes: impl Into<Arc<[u8]>>) {
        self.files.insert(path.into(), bytes.into());
    }

    fn get(&self, path: &Path) -> io::Result<&Arc<[u8]>> {
        self.files
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{}", path.display())))
    }
}

/// `Read + Seek` over shared bytes.
struct SharedBytes(Arc<[u8]>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl ContentFileSource for MemoryFiles {
    fn open(&self, path: &Path) -> io::Result<ContentFile> {
        Ok(Box::new(Cursor::new(SharedBytes(self.get(path)?.clone()))))
    }

    fn len(&self, path: &Path) -> io::Result<u64> {
        Ok(self.get(path)?.len() as u64)
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }
}
