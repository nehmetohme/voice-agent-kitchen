//! Voice Agent Kitchen server: the composition root. It is the only place
//! that reads the environment and wires implementations to ports.
//!
//! ```text
//! vak-server serve    # HTTP API on VAK_HTTP_ADDR (default 127.0.0.1:3000)
//! ```

use std::future::Future;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use vak_config::Settings;

#[derive(Debug, Parser)]
#[command(name = "vak-server", version, about = "Voice Agent Kitchen server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the HTTP API until Ctrl-C or SIGTERM, then drain in-flight requests.
    Serve,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let settings = Settings::from_lookup(|name| std::env::var(name).ok())?;
    vak_telemetry::init(&settings.log.filter)?;

    match cli.command {
        Command::Serve => serve(&settings).await,
    }
}

async fn serve(settings: &Settings) -> anyhow::Result<()> {
    let addr = settings.http.addr;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    run(listener, shutdown_signal()).await
}

/// Serves the API on `listener` until `shutdown` completes, then waits for in-flight
/// requests to finish.
async fn run(
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    axum::serve(listener, vak_api::router())
        .with_graceful_shutdown(shutdown)
        .await
        .context("serving HTTP")?;
    tracing::info!("stopped");
    Ok(())
}

/// Completes on Ctrl-C or, on Unix, SIGTERM (what Databricks Apps and Docker send).
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
    tracing::info!("shutdown signal received; draining connections");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use clap::CommandFactory as _;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpStream;
    use tokio::sync::oneshot;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn serve_is_a_subcommand() {
        let cli = Cli::try_parse_from(["vak-server", "serve"]).expect("parses");

        assert!(matches!(cli.command, Command::Serve));
    }

    #[test]
    fn a_subcommand_is_required() {
        assert!(Cli::try_parse_from(["vak-server"]).is_err());
    }

    #[tokio::test]
    async fn serves_health_then_shuts_down_gracefully() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (stop, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(run(listener, async {
            stopped.await.ok();
        }));

        let mut stream = TcpStream::connect(addr).await.expect("connect");
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("send request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .expect("read response");
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains(r#""status":"ok""#), "{response}");

        stop.send(()).expect("server is running");
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("server stops within 5 s")
            .expect("server task joins")
            .expect("server returns Ok");
    }
}
