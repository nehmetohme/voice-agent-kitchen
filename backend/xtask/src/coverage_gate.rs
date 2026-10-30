//! `coverage-gate`: per-crate line-coverage floors over `cargo llvm-cov` JSON output.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Parsed `coverage.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Floor for all instrumented lines together, in whole percent.
    pub(crate) global: Option<u32>,
    /// Floor per crate, in whole percent.
    #[serde(default)]
    pub(crate) crates: BTreeMap<String, u32>,
}

impl Config {
    pub(crate) fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// A workspace member and its absolute package directory.
#[derive(Clone, Debug)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
}

/// Line counts for one source file, from the llvm-cov summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileLines {
    pub(crate) path: PathBuf,
    pub(crate) lines: Lines,
}

/// Instrumented and covered line counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Lines {
    pub(crate) count: u64,
    pub(crate) covered: u64,
}

/// One gated unit: a crate, or the global total.
#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) name: String,
    pub(crate) lines: Lines,
    pub(crate) floor: Option<u32>,
}

/// The gate result.
#[derive(Debug, Default)]
pub(crate) struct Report {
    /// Crates with a floor or with instrumented lines, sorted by name.
    pub(crate) rows: Vec<Row>,
    /// The global total, if `coverage.toml` sets a global floor.
    pub(crate) global: Option<Row>,
    /// Problems in `coverage.toml` itself.
    pub(crate) errors: Vec<String>,
}

impl Row {
    pub(crate) fn passes(&self) -> bool {
        true
    }
}

impl Report {
    pub(crate) fn passed(&self) -> bool {
        true
    }
}

impl fmt::Display for Row {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// Reads per-file line counts from `cargo llvm-cov report --json --summary-only` output.
pub(crate) fn parse_summary(_json: &str) -> Result<Vec<FileLines>, serde_json::Error> {
    Ok(Vec::new())
}

/// Applies the floors in `config` to `files`, assigning each file to its innermost package.
pub(crate) fn gate(_config: &Config, _packages: &[Package], _files: &[FileLines]) -> Report {
    Report::default()
}
