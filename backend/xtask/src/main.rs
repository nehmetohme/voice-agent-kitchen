//! Workspace automation, run as `cargo xtask <command>` from `backend/`.
//!
//! - `check-layers`: every dependency edge in every manifest is allowed by `layers.toml`.
//! - `coverage-gate`: every crate meets its line-coverage floor in `coverage.toml`.
//!
//! Exit codes: 0 when every check passes, 1 when a check fails, 2 on a usage or I/O error.

mod coverage_gate;
mod layers;
mod workspace;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "Voice Agent Kitchen backend automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fail on any crate dependency that layers.toml does not allow.
    CheckLayers {
        /// Path to layers.toml [default: <workspace root>/layers.toml]
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Fail any crate whose line coverage is below its floor in coverage.toml.
    CoverageGate {
        /// Output of `cargo llvm-cov report --json --summary-only`
        summary: PathBuf,
        /// Path to coverage.toml [default: <workspace root>/coverage.toml]
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("xtask: error: {error:#}");
            ExitCode::from(2)
        }
    }
}

/// Runs one command; `Ok(false)` means the check ran and found problems.
fn run(cli: Cli) -> anyhow::Result<bool> {
    match cli.command {
        Command::CheckLayers { config } => check_layers(config),
        Command::CoverageGate { summary, config } => coverage_gate(&summary, config),
    }
}

fn check_layers(config: Option<PathBuf>) -> anyhow::Result<bool> {
    let metadata = workspace::metadata(None)?;
    let path = config.unwrap_or_else(|| metadata.workspace_root.join("layers.toml").into());
    let config = layers::Config::parse(&read(&path)?)
        .with_context(|| format!("parsing {}", path.display()))?;
    let crates = workspace::crates(&metadata);

    let violations = layers::check(&config, &crates);
    for violation in &violations {
        eprintln!("check-layers: {violation}");
    }
    if violations.is_empty() {
        eprintln!("check-layers: ok ({} crates)", crates.len());
    } else {
        eprintln!(
            "check-layers: {} violation(s); see backend/layers.toml",
            violations.len()
        );
    }
    Ok(violations.is_empty())
}

fn coverage_gate(summary: &Path, config: Option<PathBuf>) -> anyhow::Result<bool> {
    let metadata = workspace::metadata(None)?;
    let path = config.unwrap_or_else(|| metadata.workspace_root.join("coverage.toml").into());
    let config = coverage_gate::Config::parse(&read(&path)?)
        .with_context(|| format!("parsing {}", path.display()))?;
    let files = coverage_gate::parse_summary(&read(summary)?)
        .with_context(|| format!("parsing llvm-cov summary {}", summary.display()))?;

    let report = coverage_gate::gate(&config, &workspace::packages(&metadata), &files);
    for row in report.rows.iter().chain(&report.global) {
        eprintln!("coverage-gate: {row}");
    }
    for error in &report.errors {
        eprintln!("coverage-gate: error: {error}");
    }
    let passed = report.passed();
    if passed {
        eprintln!("coverage-gate: ok");
    } else {
        eprintln!(
            "coverage-gate: failed; raise coverage (floors in backend/coverage.toml only go up)"
        );
    }
    Ok(passed)
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}
