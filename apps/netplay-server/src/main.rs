//! `netplay-server`: reads the environment, serves on `NETPLAY_BIND` until SIGTERM / Ctrl+C.
#![deny(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use netplay_server::app_state::AppState;
use netplay_server::rate_limits::RateLimits;
use netplay_server::room_gc::spawn_room_gc;
use netplay_server::server_config::ServerConfig;
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

/// Open WebSockets get this long to answer the Close at shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let config = ServerConfig::from_env()?;
    tracing::info!(?config, "starting netplay server");
    let limits = RateLimits::new(&config.trusted_proxies);
    let bind = config.bind;
    let (stop_tx, stop_rx) = watch::channel(false);
    let stop_tx = Arc::new(stop_tx);
    let state = AppState::new(config, stop_rx);
    spawn_room_gc(Arc::clone(&state.registry), limits.clone());
    let app = netplay_server::build_router(state, &limits);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot listen on {bind}"))?;
    let signal_tx = Arc::clone(&stop_tx);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        stop_signal().await;
        tracing::info!("shutting down");
        signal_tx.send_replace(true);
    })
    .await
    .context("server failed")?;
    // Every WebSocket task holds a receiver; wait (bounded) for them to close.
    let _ = tokio::time::timeout(SHUTDOWN_GRACE, stop_tx.closed()).await;
    Ok(())
}

/// Ctrl+C anywhere, SIGTERM on Unix (`docker stop`).
async fn stop_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    {
        let mut term =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(term) => term,
                Err(_) => return ctrl_c.await,
            };
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await;
}
