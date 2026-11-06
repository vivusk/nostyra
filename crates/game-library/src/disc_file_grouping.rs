//! Multi-file disc games of one walk: the grouping rules live in
//! `rom_identity::group_disc_files` (shared with the web edition); this applies the
//! result to the walk — hidden files and failures go, a sheet's fingerprint covers
//! its files (size = sum, mtime = newest).

use std::path::Path;
use std::time::UNIX_EPOCH;

use rom_identity::{group_disc_files as group, DiscGroupCandidate};
use system_catalog::{Media, SystemSpec};

use crate::folder_scanner::DiscoveredFiles;

/// Groups the disc games in `found` (one walk of one library root).
pub(crate) fn group_disc_files(found: &mut DiscoveredFiles, systems: &[&'static SystemSpec]) {
    let is_disc_ext = |ext: &str| {
        systems
            .iter()
            .any(|s| s.media == Media::Disc && s.has_extension(ext))
    };
    let grouping = {
        let files: Vec<DiscGroupCandidate<'_>> = found
            .files
            .iter()
            .map(|f| DiscGroupCandidate {
                path: Path::new(&f.path),
                disc_classified: f.system.media == Media::Disc,
            })
            .collect();
        let failures: Vec<&Path> = found.failures.iter().map(|f| f.path.as_path()).collect();
        group(&files, &failures, &is_disc_ext)
    };
    for (index, members) in &grouping.members {
        let file = &mut found.files[*index];
        for path in members {
            if let Ok(meta) = std::fs::metadata(path) {
                file.size += meta.len();
                file.mtime_ns = file.mtime_ns.max(mtime_ns(&meta));
            }
        }
    }
    retain_unhidden(&mut found.files, &grouping.hidden_files);
    retain_unhidden(&mut found.failures, &grouping.hidden_failures);
}

/// Drops the items at `hidden` (indices), keeping the order of the rest.
fn retain_unhidden<T>(items: &mut Vec<T>, hidden: &[usize]) {
    let mut index = 0;
    items.retain(|_| {
        let keep = !hidden.contains(&index);
        index += 1;
        keep
    });
}

fn mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| {
            i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
        })
}
