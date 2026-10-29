//! Offline half of an install: zip bytes → single expected library → `*.tmp`
//! (hashed while written) → probe → atomic rename. Network-free so the whole
//! pipeline is testable from an in-memory zip.

use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use http_download_util::copy_with_limit;
use sha2::{Digest, Sha256};

use crate::buildbot_core_catalog::current_platform_core_source;
use crate::{CoreInfo, CoreManagerError, InstalledCore, MAX_CORE_BYTES};

/// Installs `core_id` from an already downloaded buildbot zip into `cores_dir`.
///
/// Only the top-level entry named exactly like the platform library is read,
/// so other entries (including `../` traversal names) are ignored. `probe`
/// must confirm the `*.tmp` file is a loadable core before it replaces any
/// existing install; on any failure the tmp file is removed and an existing
/// core is left untouched.
pub fn install_core_from_archive(
    archive: &[u8],
    core_id: &str,
    cores_dir: &Path,
    probe: impl FnOnce(&Path) -> Result<CoreInfo, String>,
    cancel: &AtomicBool,
) -> Result<InstalledCore, CoreManagerError> {
    let library_file_name = current_platform_core_source(core_id)?.library_file_name;
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
    let index = zip
        .index_for_name(&library_file_name)
        .ok_or_else(|| CoreManagerError::LibraryNotInArchive(library_file_name.clone()))?;
    let mut entry = zip.by_index(index)?;
    if !entry.is_file() {
        return Err(CoreManagerError::LibraryNotInArchive(library_file_name));
    }

    std::fs::create_dir_all(cores_dir).map_err(|error| {
        CoreManagerError::io(format!("cannot create {}", cores_dir.display()), error)
    })?;
    let final_path = cores_dir.join(&library_file_name);
    let tmp_path = cores_dir.join(format!("{library_file_name}.tmp"));

    let result = (|| {
        let sha256 = write_hashed_tmp(&mut entry, &tmp_path, cancel)?;
        let info = probe(&tmp_path).map_err(CoreManagerError::ProbeFailed)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CoreManagerError::Cancelled);
        }
        replace_core_file(&tmp_path, &final_path)?;
        Ok(InstalledCore {
            path: final_path.clone(),
            sha256,
            info,
        })
    })();

    match &result {
        Ok(core) => tracing::info!(
            core = %core.path.display(), sha256 = %core.sha256, version = %core.info.version,
            "installed libretro core"
        ),
        Err(error) => {
            let _ = std::fs::remove_file(&tmp_path); // best effort: no stray tmp file
            tracing::warn!(core_id, %error, "core install failed");
        }
    }
    result
}

/// Streams the zip entry into `tmp_path` (size-capped, cancellable) and
/// returns the lower-case hex SHA256 of what was written.
fn write_hashed_tmp(
    entry: &mut impl std::io::Read,
    tmp_path: &Path,
    cancel: &AtomicBool,
) -> Result<String, CoreManagerError> {
    let context = format!("cannot write {}", tmp_path.display());
    let file = File::create(tmp_path).map_err(|error| CoreManagerError::io(&context, error))?;
    let mut writer = HashingWriter {
        file,
        hasher: Sha256::new(),
    };
    copy_with_limit(entry, &mut writer, MAX_CORE_BYTES, cancel, |_| {}, &context)?;
    writer
        .file
        .sync_all()
        .map_err(|error| CoreManagerError::io(&context, error))?;
    Ok(writer
        .hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Atomically swaps the verified tmp file into place. On Windows a library
/// loaded by a running runner cannot be replaced → `CoreInUse`.
fn replace_core_file(tmp_path: &Path, final_path: &Path) -> Result<(), CoreManagerError> {
    // std::fs::rename replaces an existing target on Windows and Unix.
    std::fs::rename(tmp_path, final_path).map_err(|error| {
        if is_file_in_use(&error) && final_path.is_file() {
            CoreManagerError::CoreInUse {
                path: PathBuf::from(final_path),
            }
        } else {
            CoreManagerError::io(format!("cannot replace {}", final_path.display()), error)
        }
    })
}

/// ERROR_ACCESS_DENIED (5) / ERROR_SHARING_VIOLATION (32) are what Windows
/// returns when the target DLL is mapped by another process.
fn is_file_in_use(error: &std::io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5) | Some(32))
}

/// Writes to the tmp file and feeds the same bytes to SHA256 (one pass).
struct HashingWriter {
    file: File,
    hasher: Sha256,
}

impl Write for HashingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.file.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
