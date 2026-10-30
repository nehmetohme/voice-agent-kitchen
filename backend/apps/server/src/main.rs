//! Voice Agent Kitchen server: the composition root. It is the only place
//! that reads the environment and wires implementations to ports.
//!
//! ```text
//! vak-server serve                    # HTTP API on VAK_HTTP_ADDR (default 127.0.0.1:3000);
//!                                     # needs DATABASE_URL
//! vak-server migrate                  # apply the database migrations, then exit
//! vak-server openapi [--output FILE]  # the OpenAPI document (`just openapi` writes
//!                                     # contracts/openapi.json with it)
//! ```

use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use axum::Router;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use vak_api::AppState;
use vak_config::{DATABASE_URL_VAR, DEV_OPERATOR_VAR, Settings};
use vak_domain::Actor;
use vak_repositories_pg::{Database, DatabaseConfig};
use vak_services::AgentService;

/// The user ID the local dev operator acts under.
const DEV_OPERATOR_ID: &str = "dev-operator";

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
    /// Apply every pending database migration, then exit. Run it before each deploy.
    Migrate,
    /// Write the OpenAPI 3.1 document of the HTTP API. Reads no settings or environment.
    Openapi {
        /// Output file [default: stdout]
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve => {
            let settings = Settings::from_lookup(|name| std::env::var(name).ok())?;
            vak_telemetry::init(&settings.log.filter)?;
            serve(&settings).await
        }
        Command::Migrate => {
            let settings = Settings::from_lookup(|name| std::env::var(name).ok())?;
            vak_telemetry::init(&settings.log.filter)?;
            migrate(&settings).await
        }
        Command::Openapi { output } => openapi(output.as_deref()),
    }
}

/// The Postgres pool. It connects on first use, so `/health` answers while Postgres starts.
fn database(settings: &Settings) -> anyhow::Result<Database> {
    let url = settings.database.url.clone().with_context(|| {
        format!("{DATABASE_URL_VAR} is not set; serve and migrate need Postgres")
    })?;
    Database::connect_lazy(&DatabaseConfig::new(url)).context("configuring the database pool")
}

/// The application: Postgres repositories behind the services, behind the API.
fn app(settings: &Settings, database: &Database) -> anyhow::Result<Router> {
    let agents = AgentService::new(Arc::new(database.agents()));
    let mut state = AppState::new(agents);
    if let Some(email) = &settings.auth.dev_operator_email {
        let operator = Actor::new(DEV_OPERATOR_ID, email)
            .with_context(|| format!("{DEV_OPERATOR_VAR} is not a valid email address"))?;
        tracing::warn!(
            "local dev operator enabled: API requests without identity headers act as it"
        );
        state = state.with_dev_operator(operator);
    }
    Ok(vak_api::app(state))
}

async fn migrate(settings: &Settings) -> anyhow::Result<()> {
    database(settings)?
        .migrate()
        .await
        .context("applying the database migrations")?;
    tracing::info!("migrations applied");
    Ok(())
}

/// Writes the OpenAPI document to `output`, or to stdout.
fn openapi(output: Option<&Path>) -> anyhow::Result<()> {
    match output {
        Some(path) => {
            let mut file = std::fs::File::create(path)
                .with_context(|| format!("creating {}", path.display()))?;
            write_openapi(&mut file).with_context(|| format!("writing {}", path.display()))
        }
        None => write_openapi(&mut std::io::stdout().lock()).context("writing to stdout"),
    }
}

fn write_openapi(out: &mut impl Write) -> anyhow::Result<()> {
    let document = vak_api::openapi_json().context("rendering the OpenAPI document")?;
    out.write_all(document.as_bytes())?;
    out.flush()?;
    Ok(())
}

async fn serve(settings: &Settings) -> anyhow::Result<()> {
    let app = app(settings, &database(settings)?)?;
    let addr = settings.http.addr;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    run(listener, app, shutdown_signal()).await
}

/// Serves `app` on `listener` until `shutdown` completes, then waits for in-flight requests
/// to finish.
async fn run(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    axum::serve(listener, app)
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

    #[test]
    fn migrate_is_a_subcommand() {
        let cli = Cli::try_parse_from(["vak-server", "migrate"]).expect("parses");

        assert!(matches!(cli.command, Command::Migrate));
    }

    fn settings(vars: &[(&str, &str)]) -> Settings {
        Settings::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
        .expect("valid settings")
    }

    const LAZY_DB: (&str, &str) = ("DATABASE_URL", "postgres://vak@127.0.0.1:1/vak");

    #[tokio::test]
    async fn the_database_needs_database_url_and_never_echoes_it() {
        let missing = database(&settings(&[])).expect_err("no DATABASE_URL");
        let malformed = database(&settings(&[("DATABASE_URL", "postgres://u:hunter2@[x")]))
            .expect_err("malformed");

        assert!(format!("{missing:#}").contains("DATABASE_URL is not set"));
        assert!(
            !format!("{malformed:#}").contains("hunter2"),
            "{malformed:#}"
        );
        assert!(database(&settings(&[LAZY_DB])).is_ok(), "connects lazily");
    }

    #[tokio::test]
    async fn the_dev_operator_must_be_an_email_address() {
        let settings = settings(&[LAZY_DB, ("VAK_DEV_OPERATOR_EMAIL", "nobody")]);
        let database = database(&settings).expect("lazy");

        let error = app(&settings, &database).expect_err("not an email");

        assert!(format!("{error:#}").contains("VAK_DEV_OPERATOR_EMAIL"));
    }

    /// Sends one request to a running server and returns the raw HTTP response.
    async fn get(addr: std::net::SocketAddr, path: &str) -> String {
        let mut stream = TcpStream::connect(addr).await.expect("connect");
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("send request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .expect("read response");
        response
    }

    #[tokio::test]
    async fn the_api_needs_an_identity_unless_a_dev_operator_is_configured() {
        let strict = settings(&[LAZY_DB]);
        let local = settings(&[LAZY_DB, ("VAK_DEV_OPERATOR_EMAIL", "dev@localhost")]);
        for (settings, expected) in [(strict, "401 Unauthorized"), (local, "200 OK")] {
            let app = app(&settings, &database(&settings).expect("lazy")).expect("app");
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("local addr");
            let (stop, stopped) = oneshot::channel::<()>();
            let server = tokio::spawn(run(listener, app, async {
                stopped.await.ok();
            }));

            let response = get(addr, "/api/v1/catalog/voices").await;

            assert!(
                response.starts_with(&format!("HTTP/1.1 {expected}")),
                "{response}"
            );
            stop.send(()).expect("server is running");
            server.await.expect("joins").expect("stops cleanly");
        }
    }

    #[test]
    fn openapi_is_a_subcommand_with_an_optional_output_file() {
        let to_file =
            Cli::try_parse_from(["vak-server", "openapi", "--output", "out.json"]).expect("parses");
        let to_stdout = Cli::try_parse_from(["vak-server", "openapi"]).expect("parses");

        assert!(matches!(
            to_file.command,
            Command::Openapi { output: Some(ref path) } if path.as_path() == Path::new("out.json")
        ));
        assert!(matches!(
            to_stdout.command,
            Command::Openapi { output: None }
        ));
    }

    #[test]
    fn openapi_writes_the_contract_document_to_the_output_file() {
        let path =
            std::env::temp_dir().join(format!("vak-server-openapi-{}.json", std::process::id()));

        openapi(Some(&path)).expect("writes the document");

        let written = std::fs::read_to_string(&path).expect("the file exists");
        std::fs::remove_file(&path).ok();
        assert_eq!(written, vak_api::openapi_json().expect("renders"));
    }

    #[test]
    fn openapi_writes_the_same_bytes_to_any_writer() {
        let mut out = Vec::new();

        write_openapi(&mut out).expect("writes");

        assert_eq!(
            String::from_utf8(out).expect("UTF-8"),
            vak_api::openapi_json().expect("renders")
        );
    }

    #[tokio::test]
    async fn serves_health_then_shuts_down_gracefully() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (stop, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(run(listener, vak_api::router(), async {
            stopped.await.ok();
        }));

        let response = get(addr, "/health").await;
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
