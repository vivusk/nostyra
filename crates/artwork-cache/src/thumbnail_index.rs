//! Which thumbnail files exist, from one GitHub tree API response, so the
//! normal path never probes the server with 404s. Loading and refreshing
//! the cached copy lives in `thumbnail_index_refresh`.

use std::collections::HashSet;

use crate::{ArtworkError, ThumbnailFolder};

/// File names (sanitised, without `.png`) present in each thumbnail folder.
#[derive(Debug, Clone, Default)]
pub struct ThumbnailIndex {
    boxarts: HashSet<String>,
    snaps: HashSet<String>,
    titles: HashSet<String>,
}

impl ThumbnailIndex {
    /// Parses a GitHub tree response (`tree[].path` like
    /// `Named_Boxarts/<name>.png`). Files in nested folders are ignored (the
    /// server would not serve them under the plain name). An index without
    /// any box art is rejected: trusting it would mark every game "not found".
    pub fn parse_tree_json(bytes: &[u8]) -> Result<Self, ArtworkError> {
        #[derive(serde::Deserialize)]
        struct Tree {
            tree: Vec<Entry>,
            #[serde(default)]
            truncated: bool,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            path: String,
            #[serde(rename = "type")]
            kind: String,
        }

        let tree: Tree = serde_json::from_slice(bytes)
            .map_err(|error| ArtworkError::InvalidIndex(error.to_string()))?;
        if tree.truncated {
            tracing::warn!("GitHub thumbnail tree is truncated; some art will be fetched directly");
        }
        let mut index = Self::default();
        for entry in tree.tree.into_iter().filter(|entry| entry.kind == "blob") {
            let Some((dir, file)) = entry.path.split_once('/') else {
                continue;
            };
            let Some(name) = file.strip_suffix(".png").filter(|name| !name.contains('/')) else {
                continue;
            };
            let set = match dir {
                "Named_Boxarts" => &mut index.boxarts,
                "Named_Snaps" => &mut index.snaps,
                "Named_Titles" => &mut index.titles,
                _ => continue,
            };
            set.insert(name.to_owned());
        }
        if index.boxarts.is_empty() {
            return Err(ArtworkError::InvalidIndex(
                "no Named_Boxarts entries".into(),
            ));
        }
        Ok(index)
    }

    /// Number of files indexed in `folder`.
    pub fn len(&self, folder: ThumbnailFolder) -> usize {
        self.names(folder).len()
    }

    /// True when `<folder>/<file_name>.png` exists upstream.
    pub fn contains(&self, folder: ThumbnailFolder, file_name: &str) -> bool {
        self.names(folder).contains(file_name)
    }

    /// First of `candidates` (best first, see `resolve_names`) present in `folder`.
    pub fn find_name<'a>(
        &self,
        folder: ThumbnailFolder,
        candidates: &'a [String],
    ) -> Option<&'a str> {
        let names = self.names(folder);
        candidates
            .iter()
            .map(String::as_str)
            .find(|name| names.contains(*name))
    }

    fn names(&self, folder: ThumbnailFolder) -> &HashSet<String> {
        match folder {
            ThumbnailFolder::Boxarts => &self.boxarts,
            ThumbnailFolder::Snaps => &self.snaps,
            ThumbnailFolder::Titles => &self.titles,
        }
    }
}
