//! The [`CloudStorageProvider`] calls of [`FakeCloudProvider`]: each logs itself,
//! runs the hook, applies scripted faults, then works on the in-memory map.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use super::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::{CloudStorageProvider, ListEntry, Rev};

/// Chunk size of a streamed read (`read_to`).
const READ_CHUNK_BYTES: usize = 1024 * 1024;

impl FakeCloudProvider {
    /// Logs the call, runs the hook, then applies faults.
    fn begin(&self, op: FakeOp, path: &str) -> Result<(), CloudError> {
        self.lock().calls.push((op, path.to_string()));
        self.run_hook(op, path);
        let mut state = self.lock();
        if state.offline {
            return Err(CloudError::Offline);
        }
        if let Some((left, error)) = state.nth.take() {
            if left <= 1 {
                return Err(error);
            }
            state.nth = Some((left - 1, error));
        }
        let hit = state.faults.iter_mut().find(|fault| {
            fault.remaining > 0
                && fault.op.is_none_or(|want| want == op)
                && path.contains(&fault.path_contains)
        });
        if let Some(fault) = hit {
            fault.remaining -= 1;
            return Err(fault.error.clone());
        }
        Ok(())
    }

    fn run_hook(&self, op: FakeOp, path: &str) {
        let hook = self.hook.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(hook) = hook {
            hook(op, path);
        }
    }

    fn store(&self, path: &str, bytes: Vec<u8>, expect: Option<&Rev>) -> Result<Rev, CloudError> {
        let mut state = self.lock();
        if let (Some(expect), true) = (expect, state.conditional) {
            let current = state.files.get(path).map(|(_, rev)| rev_of(*rev));
            if current.as_ref() != Some(expect) {
                return Err(CloudError::RevMismatch);
            }
        }
        let rev = state.next_rev;
        state.next_rev += 1;
        state.files.insert(path.to_string(), (bytes, rev));
        Ok(rev_of(rev))
    }
}

impl CloudStorageProvider for FakeCloudProvider {
    fn list(&self, dir: &str) -> Result<Vec<ListEntry>, CloudError> {
        self.begin(FakeOp::List, dir)?;
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut entries: BTreeMap<String, bool> = BTreeMap::new();
        for key in self.lock().files.keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                let (name, is_dir) = rest
                    .split_once('/')
                    .map_or((rest, false), |(n, _)| (n, true));
                *entries.entry(name.to_string()).or_default() |= is_dir;
            }
        }
        if entries.is_empty() {
            return Err(CloudError::NotFound);
        }
        Ok(entries
            .into_iter()
            .map(|(name, is_dir)| ListEntry { name, is_dir })
            .collect())
    }

    fn read(&self, path: &str) -> Result<(Vec<u8>, Rev), CloudError> {
        self.begin(FakeOp::Read, path)?;
        let state = self.lock();
        let (bytes, rev) = state.files.get(path).ok_or(CloudError::NotFound)?;
        Ok((bytes.clone(), rev_of(*rev)))
    }

    fn read_to(&self, path: &str, out: &mut dyn Write) -> Result<Rev, CloudError> {
        self.begin(FakeOp::Read, path)?;
        let (bytes, rev) = self
            .lock()
            .files
            .get(path)
            .cloned()
            .ok_or(CloudError::NotFound)?;
        for (n, chunk) in bytes.chunks(READ_CHUNK_BYTES).enumerate() {
            if n > 0 {
                self.run_hook(FakeOp::ReadChunk, path);
            }
            out.write_all(chunk)
                .map_err(|err| CloudError::Other(format!("download aborted: {err}")))?;
        }
        Ok(rev_of(rev))
    }

    fn write(&self, path: &str, bytes: &[u8], expect: Option<&Rev>) -> Result<Rev, CloudError> {
        self.begin(FakeOp::Write, path)?;
        self.store(path, bytes.to_vec(), expect)
    }

    fn write_large(&self, path: &str, reader: &mut dyn Read, len: u64) -> Result<Rev, CloudError> {
        self.begin(FakeOp::WriteLarge, path)?;
        let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
        reader
            .read_to_end(&mut bytes)
            .map_err(|err| CloudError::Other(format!("upload aborted: {err}")))?;
        self.store(path, bytes, None)
    }

    fn delete(&self, path: &str) -> Result<(), CloudError> {
        self.begin(FakeOp::Delete, path)?;
        self.lock().files.remove(path);
        Ok(())
    }

    fn delete_dir(&self, dir: &str) -> Result<(), CloudError> {
        self.begin(FakeOp::DeleteDir, dir)?;
        let prefix = format!("{dir}/");
        self.lock().files.retain(|key, _| !key.starts_with(&prefix));
        Ok(())
    }

    fn account_label(&self) -> String {
        "fake".to_string()
    }
}

fn rev_of(rev: u64) -> Rev {
    Rev(format!("r{rev}"))
}
