//! Google Drive (API v3, scope `drive.file`): Nostyra's data lives in a visible
//! "Nostyra" folder in My Drive (Q9), under `nostyra-cloud/v1/`. Path → id
//! resolution and the folder cache are in `google_drive_path_ids`, uploads in
//! `google_drive_uploads`, downloads in `google_drive_downloads`. Drive v3 ignores `If-Match` (phase 00), so a conditional
//! write re-reads `headRevisionId` just before the PATCH; the race window left is
//! covered by the planner's "not my commit" rule.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use super::google_drive_path_ids::split_path;
use super::google_drive_requests::DriveFile;
use super::google_drive_uploads::{rev_of, UploadTarget};
use crate::http_transport::RequestBudget;
use crate::metadata_read_cap::{metadata_too_large, CappedBuffer, MAX_METADATA_BYTES};
use crate::oauth_token_session::OAuthSession;
use crate::{CloudError, CloudStorageProvider, ListEntry, Rev};

/// Name of the folder created in My Drive (found by its marker, not its name).
pub const ROOT_FOLDER_NAME: &str = "Nostyra";
/// Folders inside it that hold the sync data (same layout as the Local folder).
pub const REMOTE_ROOT_SUBDIR: [&str; 2] = ["nostyra-cloud", "v1"];

/// A signed-in Google Drive account.
pub struct GoogleDriveProvider {
    pub(crate) session: Arc<OAuthSession>,
    pub(crate) budget: RequestBudget,
    /// `nostyra-cloud/v1` (a throw-away sub-root in the live test).
    pub(crate) root_subdir: String,
    /// Shared with [`CloudStorageProvider::budgeted`] views.
    folders: Arc<Mutex<HashMap<String, String>>>,
    label: String,
}

impl GoogleDriveProvider {
    /// Provider over `session` (background budget); `label` = the stored account label.
    pub fn new(session: OAuthSession, label: impl Into<String>) -> Self {
        Self {
            session: Arc::new(session),
            budget: RequestBudget::BACKGROUND,
            root_subdir: REMOTE_ROOT_SUBDIR.join("/"),
            folders: Arc::default(),
            label: label.into(),
        }
    }

    pub(crate) fn folders(&self) -> MutexGuard<'_, HashMap<String, String>> {
        self.folders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The newest file at `path` (`NotFound` when absent).
    fn existing(&self, path: &str) -> Result<DriveFile, CloudError> {
        self.find_file(path)?
            .into_iter()
            .next()
            .ok_or(CloudError::NotFound)
    }

    /// Where `path` is written: its copies newest first (empty = new file in
    /// `parent`, folders created on the way) and the file name.
    fn upload_target(&self, path: &str) -> Result<(Vec<DriveFile>, String, String), CloudError> {
        let (dir, name) = split_path(path)?;
        let parent = self.dir_id(dir, true)?.ok_or(CloudError::NotFound)?;
        let copies = self.find_named(&parent, name, false)?;
        Ok((copies, parent, name.to_string()))
    }

    /// Best effort: same-name duplicates left by a race go once the newest is updated.
    fn drop_duplicates(&self, older: &[DriveFile]) {
        for file in older {
            if let Err(err) = self.delete_id(&file.id) {
                tracing::info!(target: "cloud_sync", %err, "drive duplicate not removed");
            }
        }
    }

    /// Runs `op`; when a cached folder vanished (another PC's GC) it re-resolves once.
    fn with_fresh_folders<T>(
        &self,
        mut op: impl FnMut() -> Result<T, CloudError>,
    ) -> Result<T, CloudError> {
        match op() {
            Err(CloudError::NotFound) => {
                self.forget_dirs(None);
                op()
            }
            other => other,
        }
    }
}

impl CloudStorageProvider for GoogleDriveProvider {
    fn list(&self, dir: &str) -> Result<Vec<ListEntry>, CloudError> {
        self.list_dir(dir)
    }

    fn read(&self, path: &str) -> Result<(Vec<u8>, Rev), CloudError> {
        let file = self.existing(path)?;
        if file.size().is_some_and(|size| size > MAX_METADATA_BYTES) {
            return Err(metadata_too_large(path));
        }
        let rev = rev_of(&file)?;
        let mut out = CappedBuffer::default();
        let download = self.download_to(&file, &mut out);
        Ok((out.finish(download, path)?, rev))
    }

    fn read_to(&self, path: &str, out: &mut dyn Write) -> Result<Rev, CloudError> {
        let file = self.existing(path)?;
        let rev = rev_of(&file)?;
        self.download_to(&file, out)?;
        Ok(rev)
    }

    fn write(&self, path: &str, bytes: &[u8], expect: Option<&Rev>) -> Result<Rev, CloudError> {
        self.with_fresh_folders(|| {
            let (copies, parent, name) = self.upload_target(path)?;
            match (copies.split_first(), expect) {
                (None, Some(_)) => Err(CloudError::RevMismatch),
                (Some((file, _)), Some(expect)) if rev_of(file).ok().as_ref() != Some(expect) => {
                    Err(CloudError::RevMismatch)
                }
                (Some((file, older)), _) => {
                    let rev =
                        self.upload_multipart(&UploadTarget::Update { id: &file.id }, bytes)?;
                    self.drop_duplicates(older);
                    Ok(rev)
                }
                (None, None) => {
                    let target = UploadTarget::Create {
                        parent: &parent,
                        name: &name,
                    };
                    self.upload_multipart(&target, bytes)
                }
            }
        })
    }

    fn write_large(&self, path: &str, reader: &mut dyn Read, len: u64) -> Result<Rev, CloudError> {
        // Nothing read from `reader` until the session is open: a vanished cached
        // folder (404 on the lookup or the session start) is re-resolved once.
        let session = self.with_fresh_folders(|| {
            let (copies, parent, name) = self.upload_target(path)?;
            let target = match copies.first() {
                Some(file) => UploadTarget::Update { id: &file.id },
                None => UploadTarget::Create {
                    parent: &parent,
                    name: &name,
                },
            };
            self.start_session(&target, len)
        })?;
        self.upload_resumable(&session, reader, len)
    }

    fn delete(&self, path: &str) -> Result<(), CloudError> {
        self.find_file(path)?
            .iter()
            .try_for_each(|file| self.delete_id(&file.id))
    }

    fn delete_dir(&self, dir: &str) -> Result<(), CloudError> {
        if dir.is_empty() {
            return Err(CloudError::Other(
                "refusing to delete the remote root".into(),
            ));
        }
        let result = match self.dir_id(dir, false)? {
            Some(id) => self.delete_folder(&id),
            None => Ok(()),
        };
        self.forget_dirs(Some(dir));
        result
    }

    fn account_label(&self) -> String {
        self.label.clone()
    }

    fn budgeted(&self, budget: RequestBudget) -> Option<Arc<dyn CloudStorageProvider>> {
        Some(Arc::new(Self {
            session: self.session.clone(),
            budget,
            root_subdir: self.root_subdir.clone(),
            folders: self.folders.clone(),
            label: self.label.clone(),
        }))
    }
}
