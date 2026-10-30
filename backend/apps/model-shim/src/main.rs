//! Dev-only model shim: serves the Databricks Model Serving invocation shapes
//! (`/serving-endpoints/{stt,tts}/invocations`, contracts/serving/) and forwards them to
//! a local mlx-audio server, so the serving adapters run unchanged in development.
//!
//! ```text
//! vak-model-shim    # env: VAK_SHIM_ADDR, VAK_SHIM_MLX_AUDIO_URL, VAK_SHIM_STT_MODEL,
//!                   #      VAK_SHIM_TTS_MODEL, VAK_SHIM_TIMEOUT_MS, VAK_LOG (see config.rs)
//! ```

mod config;
mod error;
mod mlx_audio;
mod request;
mod routes;
mod wav;

use std::sync::Arc;

use anyhow::Context as _;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::mlx_audio::MlxAudio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_lookup(|name| std::env::var(name).ok())?;
    vak_telemetry::init(&config.log_filter)?;
    let mlx_audio = MlxAudio::new(&config).context("building the mlx-audio client")?;
    let listener = TcpListener::bind(config.addr)
        .await
        .with_context(|| format!("binding {}", config.addr))?;
    tracing::info!(
        addr = %listener.local_addr()?,
        mlx_audio = %config.mlx_audio_url,
        timeout_ms = config.timeout.as_millis(),
        "listening"
    );
    axum::serve(listener, routes::router(Arc::new(mlx_audio)))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP")?;
    tracing::info!("stopped");
    Ok(())
}

/// Completes on Ctrl-C or SIGTERM (what process-compose sends).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
