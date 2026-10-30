//! Workspace automation, run as `cargo xtask <command>` from `backend/`.
//!
//! - `check-layers`: every dependency edge in every manifest is allowed by `layers.toml`.
//! - `coverage-gate`: every crate meets its line-coverage floor in `coverage.toml`.
//! - `contract-check`: every contract file matches a contract tag the owner signed.
//! - `contract-status`: each contract feature with its version and tag state.
//!
//! Exit codes: 0 when every check passes, 1 when a check fails, 2 on a usage or I/O error.

mod contracts;
mod coverage_gate;
mod git;
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
    /// Fail unless every contract file is byte-identical to a contract tag the owner signed.
    ContractCheck {
        /// Trusted ref (a pull request's base): its .github/contract-signers verifies the
        /// tags and its contracts.toml sets the minimum versions
        #[arg(long)]
        base: String,
        /// The ref under test
        #[arg(long, default_value = "HEAD")]
        head: String,
        /// Any directory inside the repository [default: the current directory]
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Local gates only: contracts awaiting the owner's signature (missing tag,
        /// untagged contract file) warn instead of failing. Violations of signed
        /// contracts still fail. CI runs without it.
        #[arg(long)]
        allow_pending: bool,
    },
    /// List contract features with their versions and tag state. Never fails on findings.
    ContractStatus {
        /// Trusted ref whose .github/contract-signers verifies the tags
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// The ref to report on
        #[arg(long, default_value = "HEAD")]
        head: String,
        /// Any directory inside the repository [default: the current directory]
        #[arg(long)]
        repo: Option<PathBuf>,
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
        Command::ContractCheck {
            base,
            head,
            repo,
            allow_pending,
        } => contract_check(&base, &head, repo.as_deref(), allow_pending),
        Command::ContractStatus { base, head, repo } => {
            contract_status(&base, &head, repo.as_deref()).map(|()| true)
        }
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

/// Resolves both refs and runs the contract checks.
fn contract_report(
    command: &str,
    base: &str,
    head: &str,
    repo: Option<&Path>,
) -> anyhow::Result<contracts::Report> {
    let git = git::Git::open(repo)?;
    let (base_id, head_id) = (git.commit(base)?, git.commit(head)?);
    eprintln!(
        "{command}: base {base} ({}), head {head} ({})",
        short(&base_id),
        short(&head_id)
    );
    let report = contracts::check(&git, &base_id, &head_id)?;
    for notice in &report.notices {
        eprintln!("{command}: notice: {notice}");
    }
    Ok(report)
}

fn contract_check(
    base: &str,
    head: &str,
    repo: Option<&Path>,
    allow_pending: bool,
) -> anyhow::Result<bool> {
    let report = contract_report("contract-check", base, head, repo)?;
    for problem in &report.problems {
        if allow_pending && problem.is_pending() {
            eprintln!("contract-check: warning: awaiting owner signature: {problem}");
        } else {
            eprintln!("contract-check: error: {problem}");
        }
    }
    let (pending, passed) = (report.pending(), report.passed(allow_pending));
    let violations = report.problems.len() - pending;
    if passed && pending == 0 {
        eprintln!(
            "contract-check: ok ({} feature(s), {} file(s) match their signed tags)",
            report.features.len(),
            report.pinned
        );
    } else if passed {
        eprintln!(
            "contract-check: ok locally, {pending} finding(s) awaiting the owner's signature \
             (see `just contract-status`). CI is strict and fails until the owner signs."
        );
    } else {
        eprintln!(
            "contract-check: failed: {violations} violation(s), {pending} awaiting the owner's \
             signature{}. Contract files stay byte-identical to the tags the owner signed; \
             see contracts/CLAUDE.md",
            if allow_pending { " (allowed)" } else { "" }
        );
    }
    Ok(passed)
}

fn contract_status(base: &str, head: &str, repo: Option<&Path>) -> anyhow::Result<()> {
    let report = contract_report("contract-status", base, head, repo)?;
    let header = ["feature", "version", "tag", "state"].map(str::to_owned);
    let rows: Vec<[String; 4]> = report
        .features
        .iter()
        .map(|f| {
            [
                f.name.clone(),
                f.version.to_string(),
                f.tag(),
                f.state.clone(),
            ]
        })
        .collect();
    let mut widths = header.clone().map(|cell| cell.len());
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let [w0, w1, w2, _] = widths;
    for [name, version, tag, state] in std::iter::once(&header).chain(&rows) {
        eprintln!("{name:<w0$}  {version:<w1$}  {tag:<w2$}  {state}");
    }
    if rows.is_empty() {
        eprintln!("(no features in {})", contracts::MANIFEST);
    }
    for problem in &report.problems {
        if matches!(problem, contracts::Problem::Untagged { .. }) {
            eprintln!("contract-status: {problem}");
        }
    }
    let pending = report.pending();
    eprintln!(
        "contract-status: {pending} finding(s) awaiting the owner's signature, {} violation(s). \
         Local gates fail only on violations; CI fails on both.",
        report.problems.len() - pending
    );
    Ok(())
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}
