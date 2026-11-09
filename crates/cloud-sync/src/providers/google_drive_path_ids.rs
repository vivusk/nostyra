//! Drive is id-based: our `/` paths resolve folder by folder from the "Nostyra"
//! folder (found by its `appProperties` marker, so a renamed folder still works)
//! through `nostyra-cloud/v1/…`. Folder ids are cached for the session (files are
//! looked up fresh: their revision matters); missing folders are created on demand.
//! Same-name duplicates (Drive allows them after a race): the oldest folder wins on
//! every PC, the newest file is read and updated.

use http_download_util::HttpMethod;
use serde_json::json;

use super::google_drive_errors::escape_q;
use super::google_drive_provider::{GoogleDriveProvider, ROOT_FOLDER_NAME};
use super::google_drive_requests::{DriveCall, DriveFile, DRIVE_API, FOLDER_MIME, JSON_TYPE};
use crate::sync_staging::is_safe_relative_path;
use crate::{CloudError, ListEntry};

/// `appProperties` marker of the root folder.
const ROOT_MARKER: (&str, &str) = ("nostyra", "root-v1");

/// `dir/name` → (`dir`, `name`) after the safety check.
pub(crate) fn split_path(path: &str) -> Result<(&str, &str), CloudError> {
    if !is_safe_relative_path(path) {
        return Err(CloudError::Other(format!("unsafe remote path {path:?}")));
    }
    Ok(path.rsplit_once('/').unwrap_or(("", path)))
}

impl GoogleDriveProvider {
    /// Cache key of a provider-relative dir (`""` = the v1 root): its path below the
    /// "Nostyra" folder.
    fn dir_key(&self, dir: &str) -> String {
        if dir.is_empty() {
            self.root_subdir.clone()
        } else {
            format!("{}/{dir}", self.root_subdir)
        }
    }

    /// Folder id of `dir`; `None` when it does not exist and `create` is false.
    pub(crate) fn dir_id(&self, dir: &str, create: bool) -> Result<Option<String>, CloudError> {
        if !dir.is_empty() && !is_safe_relative_path(dir) {
            return Err(CloudError::Other(format!("unsafe remote dir {dir:?}")));
        }
        let key = self.dir_key(dir);
        if let Some(id) = self.cached_folder(&key) {
            return Ok(Some(id));
        }
        let Some(mut parent) = self.root_folder_id(create)? else {
            return Ok(None);
        };
        let mut walked = String::new();
        for segment in key.split('/') {
            if !walked.is_empty() {
                walked.push('/');
            }
            walked.push_str(segment);
            if let Some(id) = self.cached_folder(&walked) {
                parent = id;
                continue;
            }
            let found = self.find_named(&parent, segment, true)?.into_iter().next();
            parent = match (found, create) {
                (Some(folder), _) => folder.id,
                (None, true) => self.create_folder(Some(&parent), segment, None)?,
                (None, false) => return Ok(None),
            };
            self.remember_folder(&walked, &parent);
        }
        Ok(Some(parent))
    }

    /// Files named like `path`'s last segment in its folder, newest first (empty
    /// when the folder or file does not exist).
    pub(crate) fn find_file(&self, path: &str) -> Result<Vec<DriveFile>, CloudError> {
        let (dir, name) = split_path(path)?;
        match self.dir_id(dir, false)? {
            Some(parent) => self.find_named(&parent, name, false),
            None => Ok(Vec::new()),
        }
    }

    /// Children of `dir` (`NotFound` when the folder is missing).
    pub(crate) fn list_dir(&self, dir: &str) -> Result<Vec<ListEntry>, CloudError> {
        let parent = self.dir_id(dir, false)?.ok_or(CloudError::NotFound)?;
        let q = format!("'{}' in parents and trashed = false", escape_q(&parent));
        let mut entries: Vec<ListEntry> = self
            .query_files(&q, "name")?
            .into_iter()
            .map(|file| ListEntry {
                is_dir: file.is_folder(),
                name: file.name,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries.dedup_by(|a, b| a.name == b.name);
        Ok(entries)
    }

    /// Drops `dir` and everything below it from the folder cache (`None` = all).
    pub(crate) fn forget_dirs(&self, dir: Option<&str>) {
        let mut folders = self.folders();
        match dir {
            None => folders.clear(),
            Some(dir) => {
                let key = self.dir_key(dir);
                let prefix = format!("{key}/");
                folders.retain(|path, _| path != &key && !path.starts_with(&prefix));
            }
        }
    }

    /// The "Nostyra" folder: by marker, else created (when `create`).
    fn root_folder_id(&self, create: bool) -> Result<Option<String>, CloudError> {
        if let Some(id) = self.cached_folder("") {
            return Ok(Some(id));
        }
        let (key, value) = ROOT_MARKER;
        let q = format!(
            "mimeType = '{FOLDER_MIME}' and trashed = false and \
             appProperties has {{ key='{key}' and value='{value}' }}"
        );
        let id = match self.query_files(&q, "createdTime")?.into_iter().next() {
            Some(folder) => folder.id,
            None if create => self.create_folder(None, ROOT_FOLDER_NAME, Some(ROOT_MARKER))?,
            None => return Ok(None),
        };
        self.remember_folder("", &id);
        Ok(Some(id))
    }

    /// Children of `parent` named `name` (folders oldest first, files newest first).
    pub(crate) fn find_named(
        &self,
        parent: &str,
        name: &str,
        folders: bool,
    ) -> Result<Vec<DriveFile>, CloudError> {
        let (op, order) = if folders {
            ("=", "createdTime")
        } else {
            ("!=", "modifiedTime desc")
        };
        let q = format!(
            "name = '{}' and '{}' in parents and trashed = false and mimeType {op} '{FOLDER_MIME}'",
            escape_q(name),
            escape_q(parent)
        );
        self.query_files(&q, order)
    }

    /// `files.create` of a folder (in My Drive's root when `parent` is `None`).
    fn create_folder(
        &self,
        parent: Option<&str>,
        name: &str,
        marker: Option<(&str, &str)>,
    ) -> Result<String, CloudError> {
        let mut meta = json!({ "name": name, "mimeType": FOLDER_MIME });
        if let Some(parent) = parent {
            meta["parents"] = json!([parent]);
        }
        if let Some((key, value)) = marker {
            meta["appProperties"] = json!({ key: value });
        }
        let body = meta.to_string();
        let url = format!("{DRIVE_API}/files?fields=id");
        let call = DriveCall::new(
            HttpMethod::Post,
            url,
            Some(JSON_TYPE),
            body.as_bytes(),
            false,
        );
        let folder: DriveFile = self.send_json(&call)?;
        Ok(folder.id)
    }

    fn cached_folder(&self, key: &str) -> Option<String> {
        self.folders().get(key).cloned()
    }

    fn remember_folder(&self, key: &str, id: &str) {
        self.folders().insert(key.to_string(), id.to_string());
    }
}
