//! The "Local folder" provider: a folder the user's Drive / OneDrive / Dropbox
//! desktop client already syncs (also the end-to-end test provider). Remote paths
//! live under `<folder>/nostyra-cloud/v1/`. `Rev` = sha256 of the file, so
//! conditional writes work as a check-then-write (a desktop client is
//! last-writer-wins anyway; the planner's "not my commit" rule covers that).
//! Client conflict copies (`… (conflicted copy) …`, OneDrive `…-<PCNAME>.…`) are
//! never read: manifests are read by exact name and `list` hides them.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::{CloudStorageProvider, ListEntry, Rev};
use crate::content_hash::{sha256_hex, to_hex};
use crate::device_identity::current_computer_name;
use crate::metadata_read_cap::read_file_capped;
use crate::sync_staging::{is_safe_relative_path, join_rel};

/// Folder inside the user's folder that holds Nostyra's data.
pub const LOCAL_FOLDER_SUBDIR: [&str; 2] = ["nostyra-cloud", "v1"];

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

/// Storage in a plain folder.
#[derive(Debug, Clone)]
pub struct LocalFolderProvider {
    folder: PathBuf,
    root: PathBuf,
}

impl LocalFolderProvider {
    /// Provider over `folder` (created lazily on the first write).
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        let folder = folder.into();
        let root = LOCAL_FOLDER_SUBDIR
            .iter()
            .fold(folder.clone(), |path, seg| path.join(seg));
        Self { folder, root }
    }

    /// `<folder>/nostyra-cloud/v1`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, CloudError> {
        if path.is_empty() {
            return Ok(self.root.clone());
        }
        if !is_safe_relative_path(path) {
            return Err(CloudError::Other(format!("unsafe remote path {path:?}")));
        }
        Ok(join_rel(&self.root, path))
    }
}

impl CloudStorageProvider for LocalFolderProvider {
    fn list(&self, dir: &str) -> Result<Vec<ListEntry>, CloudError> {
        let read = std::fs::read_dir(self.resolve(dir)?).map_err(map_io)?;
        let pc = current_computer_name().to_ascii_lowercase();
        let mut entries = Vec::new();
        for entry in read {
            let entry = entry.map_err(map_io)?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if is_client_artifact(&name, &pc) {
                continue;
            }
            let is_dir = entry.file_type().map_err(map_io)?.is_dir();
            entries.push(ListEntry { name, is_dir });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn read(&self, path: &str) -> Result<(Vec<u8>, Rev), CloudError> {
        let bytes = read_file_capped(&self.resolve(path)?).map_err(map_io)?;
        let rev = Rev(sha256_hex(&bytes));
        Ok((bytes, rev))
    }

    fn read_to(&self, path: &str, out: &mut dyn Write) -> Result<Rev, CloudError> {
        let mut file = std::fs::File::open(self.resolve(path)?).map_err(map_io)?;
        copy_hashed(&mut file, out).map(Rev).map_err(map_io)
    }

    fn write(&self, path: &str, bytes: &[u8], expect: Option<&Rev>) -> Result<Rev, CloudError> {
        let target = self.resolve(path)?;
        if let Some(expect) = expect {
            match read_file_capped(&target) {
                Ok(current) if sha256_hex(&current) == expect.0 => {}
                Ok(_) => return Err(CloudError::RevMismatch),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    return Err(CloudError::RevMismatch)
                }
                Err(err) => return Err(map_io(err)),
            }
        }
        create_parent(&target)?;
        config_layers::write_atomically(&target, bytes).map_err(map_io)?;
        Ok(Rev(sha256_hex(bytes)))
    }

    fn write_large(&self, path: &str, reader: &mut dyn Read, _len: u64) -> Result<Rev, CloudError> {
        let target = self.resolve(path)?;
        create_parent(&target)?;
        let tmp = target.with_file_name(format!("{}.tmp", file_name(&target)));
        let result = std::fs::File::create(&tmp).and_then(|mut file| {
            let sha = copy_hashed(reader, &mut file)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&tmp, &target)?;
            Ok(sha)
        });
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp); // best effort: no stray temp file
        }
        result.map(Rev).map_err(map_io)
    }

    fn delete(&self, path: &str) -> Result<(), CloudError> {
        ok_if_missing(std::fs::remove_file(self.resolve(path)?))
    }

    fn delete_dir(&self, dir: &str) -> Result<(), CloudError> {
        if dir.is_empty() {
            return Err(CloudError::Other(
                "refusing to delete the remote root".into(),
            ));
        }
        ok_if_missing(std::fs::remove_dir_all(self.resolve(dir)?))
    }

    fn account_label(&self) -> String {
        self.folder.display().to_string()
    }
}

/// Temp files of our atomic writes and desktop-client conflict copies.
fn is_client_artifact(name: &str, pc_lower: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower
        .rsplit_once('.')
        .map_or(lower.as_str(), |(stem, _)| stem);
    lower.ends_with(".tmp")
        || lower.contains("conflicted copy")
        || (!pc_lower.is_empty() && stem.ends_with(&format!("-{pc_lower}")))
}

/// Copies `reader` into `out` in 1 MiB chunks; sha256 of the bytes copied.
fn copy_hashed(reader: &mut dyn Read, out: &mut dyn Write) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])?;
    }
    Ok(to_hex(&hasher.finalize()))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn create_parent(path: &Path) -> Result<(), CloudError> {
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent).map_err(map_io),
        None => Ok(()),
    }
}

fn ok_if_missing(result: io::Result<()>) -> Result<(), CloudError> {
    match result {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(map_io(err)),
        _ => Ok(()),
    }
}

fn map_io(err: io::Error) -> CloudError {
    match err.kind() {
        io::ErrorKind::NotFound => CloudError::NotFound,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => CloudError::QuotaFull,
        _ => CloudError::Other(err.to_string()),
    }
}

#[cfg(test)]
#[path = "local_folder_provider_tests.rs"]
mod tests;
