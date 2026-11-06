//! Library-side helpers of the web scan beyond one file's identity: the metadat index
//! (genre / developer / year exactly like the desktop) and multi-file disc grouping,
//! both from `rom-identity`. JSON shapes are camelCase for TypeScript.

use std::path::Path;

use rom_identity::serial_match_keys::serial_rule;
use rom_identity::{
    group_disc_files, metadata_attribute_folders, stored_game_metadata, DatIndex,
    DiscGroupCandidate, MetadataIndex, MetadataRecord,
};
use serde::{Deserialize, Serialize};
use system_catalog::{system, Media, ALL_SYSTEMS};

/// A game's index metadata (`game_metadata` columns) and the version to stamp it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataView {
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<u16>,
    pub players: Option<u8>,
    pub source_version: &'static str,
}

impl MetadataView {
    pub fn new(record: &MetadataRecord, index: &MetadataIndex) -> Self {
        Self {
            developer: record.developer.clone(),
            publisher: record.publisher.clone(),
            genre: record.genre.clone(),
            release_year: record.release_year,
            players: record.players,
            source_version: index.source_version(),
        }
    }
}

/// `system_id`'s metadat index from the five attribute files' texts, in
/// [`metadata_attribute_folders`] order (`None` = not fetched: partial index).
pub fn metadata_index(
    system_id: &str,
    texts: [Option<String>; 5],
) -> Result<MetadataIndex, String> {
    let spec = system(system_id).ok_or_else(|| format!("unknown system {system_id}"))?;
    let files = metadata_attribute_folders()
        .into_iter()
        .zip(texts)
        .map(|(folder, text)| (folder, text.ok_or_else(|| format!("{folder} not loaded"))));
    Ok(MetadataIndex::from_texts(files).with_serial_rule(serial_rule(spec)))
}

/// Metadata of a stored game ("Reset to database values"; desktop lookup order).
pub fn stored_metadata(
    dat: &DatIndex,
    metadata: &MetadataIndex,
    dat_name: Option<&str>,
    crc32: &str,
    serial: Option<&str>,
) -> Option<MetadataView> {
    stored_game_metadata(dat, metadata, dat_name, crc32, serial)
        .map(|record| MetadataView::new(record, metadata))
}

/// One classified file of a web walk.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupFile {
    pub path: String,
    /// Classified as a disc system's file.
    pub disc: bool,
}

/// Grouping result by index (see `rom_identity::DiscGrouping`); member paths are
/// registered paths, so the worker can sum their sizes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingView {
    pub hidden_files: Vec<usize>,
    pub hidden_failures: Vec<usize>,
    pub members: Vec<SheetMembers>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SheetMembers {
    pub index: usize,
    pub paths: Vec<String>,
}

/// Groups the multi-file disc games of one folder walk (every registry disc system,
/// as the desktop scanner does), on the current content source.
pub fn group_files(files: &[GroupFile], failures: &[String]) -> GroupingView {
    let is_disc_ext = |ext: &str| {
        ALL_SYSTEMS
            .iter()
            .any(|s| s.media == Media::Disc && s.has_extension(ext))
    };
    let candidates: Vec<DiscGroupCandidate<'_>> = files
        .iter()
        .map(|f| DiscGroupCandidate {
            path: Path::new(&f.path),
            disc_classified: f.disc,
        })
        .collect();
    let failures: Vec<&Path> = failures.iter().map(Path::new).collect();
    let grouping = group_disc_files(&candidates, &failures, &is_disc_ext);
    GroupingView {
        hidden_files: grouping.hidden_files,
        hidden_failures: grouping.hidden_failures,
        members: grouping
            .members
            .into_iter()
            .map(|(index, paths)| SheetMembers {
                index,
                paths: paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "library_json_tests.rs"]
mod tests;
