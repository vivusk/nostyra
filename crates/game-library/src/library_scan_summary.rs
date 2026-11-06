//! What a library scan reports: progress for the UI banner and the final summary.

use storage_db::RelinkedGame;

use crate::folder_scanner::ScanFailure;

/// Progress for the UI scan banner. `files_done` counts unchanged files too; pass A
/// ("looking for games") reports each folder with `files_total == 0`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub folder_path: String,
    pub files_total: usize,
    pub files_done: usize,
    pub current_path: Option<String>,
}

/// What a scan did. `matched`/`unmatched` count the files hashed in this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub relinked: usize,
    /// Every relink of this scan (old → new path), for the launcher's save-folder move.
    pub relinked_games: Vec<RelinkedGame>,
    pub missing: usize,
    pub skipped: usize,
    pub matched: usize,
    pub unmatched: usize,
    /// Unchanged games the DAT identified this run (no re-hash needed).
    pub rematched: usize,
    pub failures: Vec<ScanFailure>,
    /// `(folder path, error)` of folders that could not be scanned (missing root,
    /// database error); their rows are left as they were.
    pub folder_errors: Vec<(String, String)>,
    pub cancelled: bool,
}
