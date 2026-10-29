//! Process-wide launcher state managed by Tauri: the database, the per-system DAT
//! and metadata catalogs, data paths, the HTTP client and the handles of the (at most
//! one each) running scan, artwork fetch, core download and game.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context;
use artwork_cache::ThumbnailIndex;
use game_library::SystemCatalogs;
use http_download_util::HttpClient;
use storage_db::Connection;
use tauri::{AppHandle, Manager};

use crate::app_events::{ArtworkProgressPayload, ScanProgressPayload};
use crate::dat_file_path::libretro_database_dir;
use crate::runner_binary_path::runner_binary_path;

/// Where everything lives (shared root with the runner, see ipc-protocol).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
    pub cores_dir: PathBuf,
    /// Cached covers/heroes + thumbnail index; served to the UI via the asset protocol.
    pub artwork_dir: PathBuf,
    pub runner: PathBuf,
}

/// The running game, if any (M1: one session at a time).
#[derive(Debug, Clone, Copy)]
pub struct ActiveSession {
    pub game_id: i64,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    /// No-Intro DAT + metadat per system, loaded on first use (startup warm-up loads
    /// the systems already in the library).
    pub catalogs: SystemCatalogs,
    pub paths: AppPaths,
    /// Cancel flag of the running scan; `None` = idle.
    pub scan_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// A scan was requested while one ran (e.g. folder added): run again when it ends.
    pub scan_rerun: AtomicBool,
    pub scan_progress: Mutex<Option<ScanProgressPayload>>,
    /// Shared HTTPS client (`NOSTYRA_OFFLINE=1` → every request fails fast as offline).
    pub http: HttpClient,
    /// libretro-thumbnails index per system id, loaded by the first artwork job that
    /// needs it.
    pub thumbnail_indexes: Mutex<HashMap<&'static str, Arc<ThumbnailIndex>>>,
    /// Cancel flag of the running artwork fetch; `None` = idle.
    pub artwork_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// "Fetch missing" requested while a job ran: rebuild the work list at its end.
    pub artwork_rerun: AtomicBool,
    /// Per-game fetches requested while a job ran, drained at its end.
    pub artwork_queued_games: Mutex<Vec<i64>>,
    pub artwork_progress: Mutex<Option<ArtworkProgressPayload>>,
    /// Cancel flag of the running core download; `None` = idle.
    pub core_download: Mutex<Option<Arc<AtomicBool>>>,
    pub session: Mutex<Option<ActiveSession>>,
}

impl AppState {
    /// Opens `<data>/nostyra.db` and imports play time left by runners while the
    /// launcher was closed. Catalogs load lazily (missing/corrupt files → empty index).
    pub fn init(app: &AppHandle) -> anyhow::Result<Self> {
        // Settings → Data folder choice, else the default (see ipc_protocol::data_root).
        let data_root =
            ipc_protocol::data_root().context("cannot determine the user data folder")?;
        let conn = storage_db::open_database(&data_root.join("nostyra.db"))
            .context("cannot open the library database")?;
        match runner_session::import_pending_sessions(&conn, &data_root) {
            Ok(summary) => tracing::info!(?summary, "imported pending play sessions"),
            Err(err) => tracing::warn!(%err, "pending play sessions not imported"),
        }
        let database_dir = match app.path().resource_dir() {
            Ok(dir) => Some(libretro_database_dir(&dir)),
            Err(err) => {
                tracing::warn!(%err, "no resource dir; game database unavailable");
                None
            }
        };
        Ok(Self {
            db: Mutex::new(conn),
            catalogs: SystemCatalogs::new(database_dir),
            paths: AppPaths {
                cores_dir: data_root.join("cores"),
                artwork_dir: data_root.join("artwork"),
                data_root,
                runner: runner_binary_path().context("cannot locate nostyra-runner")?,
            },
            scan_cancel: Mutex::new(None),
            scan_rerun: AtomicBool::new(false),
            scan_progress: Mutex::new(None),
            http: http_download_util::build_http_client(
                concat!("Nostyra/", env!("CARGO_PKG_VERSION")),
                std::env::var("NOSTYRA_OFFLINE").is_ok_and(|v| v == "1"),
            ),
            thumbnail_indexes: Mutex::new(HashMap::new()),
            artwork_cancel: Mutex::new(None),
            artwork_rerun: AtomicBool::new(false),
            artwork_queued_games: Mutex::new(Vec::new()),
            artwork_progress: Mutex::new(None),
            core_download: Mutex::new(None),
            session: Mutex::new(None),
        })
    }

    /// Database connection. A panic while holding it cannot leave SQLite half-written
    /// (transactions roll back), so a poisoned lock is safe to reuse.
    pub fn db(&self) -> MutexGuard<'_, Connection> {
        lock(&self.db)
    }
}

/// Current unix time in seconds (storage-db timestamps).
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Poison-tolerant lock for the small state mutexes.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
