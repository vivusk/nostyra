//! Starting an [`AchievementSession`]: settings, login, game identity and the
//! backend; then token login and the offline-journal replay, queued on the HTTP
//! worker. Identify-and-load (path, or the soft-patched bytes the core received, Q7)
//! follows after the first real frame, once memory regions exist (`DeferredLoad`).
//! Hardcore is read here only (launch = the fresh boot RA requires): the setting
//! counts only with the hidden UI flag, and a disallowed core option drops it.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use achievements::{
    hash_request, user_agent, AchievementClient, Credentials, HashUnavailable, LazyMemoryRegions,
    MemoryReader,
};
use config_layers::AchievementsConfig;
use system_catalog::SystemSpec;

use super::achievement_credentials;
use super::achievement_event_routing::SessionRouter;
use super::achievement_http_worker::{replay_journal_job, runner_backend, SessionBackend};
use super::hardcore_option_check::disallowed_option;
use super::offline_session_watch::OfflineSessionWatch;
use super::patched_rom_fallback::PatchedRomFallback;
use super::{AchievementSession, DeferredLoad, EventSink};
use crate::achievement_overlay::{AchievementOverlay, OverlayComposer};
use crate::hardcore_gate::HardcoreGate;
use crate::session_reporter::SessionReporter;

/// What a session needs from the launch.
pub struct SessionStart<'a> {
    pub config: &'a AchievementsConfig,
    /// Launcher env login (read and cleared at process start), else the file is read.
    pub env_credentials: Option<Credentials>,
    pub data_root: &'a Path,
    /// `ipc_protocol` content id (snapshot file name).
    pub content_id: &'a str,
    pub system: &'static SystemSpec,
    pub rom: &'a Path,
    /// Soft-patched cartridge bytes the core received (hashed instead of the file).
    pub patched: Option<Vec<u8>>,
    /// Core library file stem and version (User-Agent clause).
    pub core_stem: &'a str,
    pub core_version: &'a str,
    /// The core's `library_name` (rcheevos' disallowed-option table key).
    pub library_name: &'a str,
    /// Every option the host answers (fixed + BIOS switch + `--core-option`).
    pub core_options: &'a [(String, String)],
    /// `achievements::hardcore_ui_enabled()` (tests pass it directly).
    pub hardcore_ui: bool,
}

/// The runner's session for `start` (real transport, launcher events via `reporter`).
pub fn start_for_runner(
    start: SessionStart<'_>,
    reporter: &SessionReporter,
) -> Option<AchievementSession> {
    let reporter = reporter.clone();
    let data_root = start.data_root;
    let backend = move |ua: &str| runner_backend(ua, data_root);
    AchievementSession::start(start, backend, Box::new(move |e| reporter.emit(e)))
}

impl AchievementSession {
    /// Starts the session: token login, identify-and-load, journal replay on the
    /// worker. `None` when achievements cannot run for this launch (logged).
    pub fn start(
        mut start: SessionStart<'_>,
        backend: impl FnOnce(&str) -> SessionBackend,
        emit: EventSink,
    ) -> Option<Self> {
        if !start.config.enabled {
            tracing::info!("achievements disabled in settings");
            return None;
        }
        let credentials = achievement_credentials::session_credentials(
            start.env_credentials.take(),
            start.data_root,
        )?;
        let request = match hash_request(start.system, start.rom, start.patched.take()) {
            Ok(request) => request,
            Err(HashUnavailable::System) => return None,
            Err(err @ HashUnavailable::Format) => {
                tracing::info!(%err, "achievements unavailable");
                let memory = Rc::new(RefCell::new(LazyMemoryRegions::new(0)));
                let mut inert = Self::new(None, memory, &credentials, &start, emit);
                inert.router.notify(format!("RetroAchievements: {err}"));
                return Some(inert);
            }
        };
        let patched_fallback = PatchedRomFallback::arm(start.config, &request, start.rom);
        let hardcore = start.config.hardcore && start.hardcore_ui;
        if start.config.hardcore && !start.hardcore_ui {
            tracing::info!("hardcore setting ignored until RetroAchievements approval: softcore");
        }
        let ua = user_agent(start.core_stem, start.core_version);
        let backend = backend(&ua);
        let offline = backend.offline.clone(); // before the transport moves into the client
        let memory = Rc::new(RefCell::new(LazyMemoryRegions::new(request.console_id)));
        let reader_memory = memory.clone();
        let reader: MemoryReader = Box::new(move |address, buffer| {
            reader_memory
                .try_borrow()
                .map_or(0, |regions| regions.read(address, buffer))
        });
        let client = match AchievementClient::new(backend.transport, backend.dispatch, reader) {
            Ok(client) => client,
            Err(err) => {
                tracing::error!(%err, "achievements off for this session");
                return None;
            }
        };
        client.set_hardcore_enabled(hardcore);
        client.begin_login_with_token(&credentials.user, &credentials.token);
        client.run_transport_job(replay_journal_job(start.data_root, &credentials));
        tracing::info!(user = %credentials.user, hardcore, %ua, "achievement session started");
        let mut session = Self::new(Some(client), memory, &credentials, &start, emit);
        session.console_id = request.console_id;
        session.router.patched_fallback = patched_fallback;
        session.deferred = Some(DeferredLoad::Identify(request));
        session.hardcore = HardcoreGate::new(hardcore);
        session.offline = OfflineSessionWatch::new(offline);
        let core = system_catalog::core_for_system(start.system.id);
        if let Some(option) = hardcore
            .then(|| disallowed_option(start.library_name, core, start.core_options))
            .flatten()
        {
            session.drop_to_softcore(&format!("core option {option} not allowed"));
        }
        session.pump();
        Some(session)
    }

    fn new(
        client: Option<AchievementClient>,
        memory: Rc<RefCell<LazyMemoryRegions>>,
        credentials: &Credentials,
        start: &SessionStart<'_>,
        emit: EventSink,
    ) -> Self {
        let config = start.config;
        Self {
            client,
            memory,
            console_id: 0,
            rom: start.rom.to_path_buf(),
            memory_unavailable: false,
            composer: OverlayComposer::default(),
            chime: None,
            deferred: None,
            hardcore: HardcoreGate::default(),
            offline: OfflineSessionWatch::default(),
            router: SessionRouter::new(
                start.data_root,
                (start.content_id, &credentials.user),
                AchievementOverlay::new(start.data_root, config.sound, config.leaderboard_popups),
                emit,
            ),
        }
    }
}
