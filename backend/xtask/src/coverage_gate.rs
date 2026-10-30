//! `coverage-gate`: per-crate line-coverage floors over `cargo llvm-cov` JSON output.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
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

impl Lines {
    /// Coverage as `"85.7%"`, rounded down to a tenth of a percent.
    fn percent(self) -> String {
        if self.count == 0 {
            return "-".to_owned();
        }
        let tenths = self.covered.saturating_mul(1000) / self.count;
        format!("{}.{}%", tenths / 10, tenths % 10)
    }
}

impl std::ops::AddAssign for Lines {
    fn add_assign(&mut self, other: Self) {
        self.count += other.count;
        self.covered += other.covered;
    }
}

impl Row {
    /// No floor, or no instrumented lines, or `covered / count >= floor`.
    pub(crate) fn passes(&self) -> bool {
        match self.floor {
            None => true,
            Some(_) if self.lines.count == 0 => true,
            Some(floor) => {
                self.lines.covered.saturating_mul(100) >= u64::from(floor) * self.lines.count
            }
        }
    }
}

impl Report {
    pub(crate) fn passed(&self) -> bool {
        self.errors.is_empty()
            && self.rows.iter().all(Row::passes)
            && self.global.as_ref().is_none_or(Row::passes)
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Lines { count, covered } = self.lines;
        let lines = format!("{covered}/{count}");
        let (floor, status) = match self.floor {
            None => ("-".to_owned(), "no floor"),
            Some(floor) if self.passes() => (format!("{floor}%"), "ok"),
            Some(floor) => (format!("{floor}%"), "FAIL"),
        };
        write!(
            f,
            "{name:<32} {lines:>11} lines {percent:>7}  floor {floor:>4}  {status}",
            name = self.name,
            percent = self.lines.percent(),
        )
    }
}

/// The parts of the llvm-cov export format the gate reads; other fields are ignored.
#[derive(Deserialize)]
struct Export {
    data: Vec<ExportData>,
}

#[derive(Deserialize)]
struct ExportData {
    files: Vec<ExportFile>,
}

#[derive(Deserialize)]
struct ExportFile {
    filename: PathBuf,
    summary: ExportSummary,
}

#[derive(Deserialize)]
struct ExportSummary {
    lines: Lines,
}

/// Reads per-file line counts from `cargo llvm-cov report --json --summary-only` output.
pub(crate) fn parse_summary(json: &str) -> Result<Vec<FileLines>, serde_json::Error> {
    let export: Export = serde_json::from_str(json)?;
    Ok(export
        .data
        .into_iter()
        .flat_map(|data| data.files)
        .map(|file| FileLines {
            path: file.filename,
            lines: file.summary.lines,
        })
        .collect())
}

/// Applies the floors in `config` to `files`, assigning each file to its innermost package.
pub(crate) fn gate(config: &Config, packages: &[Package], files: &[FileLines]) -> Report {
    let mut per_crate: BTreeMap<&str, Lines> = BTreeMap::new();
    let mut total = Lines::default();
    for file in files {
        total += file.lines;
        if let Some(package) = innermost(packages, &file.path) {
            *per_crate.entry(package.name.as_str()).or_default() += file.lines;
        }
    }

    let mut rows: Vec<Row> = packages
        .iter()
        .filter_map(|package| {
            let floor = config.crates.get(&package.name).copied();
            let lines = per_crate.get(package.name.as_str()).copied();
            (floor.is_some() || lines.is_some()).then(|| Row {
                name: package.name.clone(),
                lines: lines.unwrap_or_default(),
                floor,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    Report {
        rows,
        global: config.global.map(|floor| Row {
            name: "global".to_owned(),
            lines: total,
            floor: Some(floor),
        }),
        errors: config_errors(config, packages),
    }
}

fn innermost<'p>(packages: &'p [Package], file: &Path) -> Option<&'p Package> {
    packages
        .iter()
        .filter(|package| file.starts_with(&package.dir))
        .max_by_key(|package| package.dir.components().count())
}

fn config_errors(config: &Config, packages: &[Package]) -> Vec<String> {
    let members: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut errors = Vec::new();
    let range = "floors are whole percents from 0 to 100";
    if let Some(floor) = config.global.filter(|floor| *floor > 100) {
        errors.push(format!("coverage.toml global = {floor}: {range}"));
    }
    for (name, floor) in &config.crates {
        if !members.contains(name.as_str()) {
            errors.push(format!(
                "coverage.toml [crates] {name}: not a workspace member"
            ));
        }
        if *floor > 100 {
            errors.push(format!("coverage.toml [crates] {name} = {floor}: {range}"));
        }
    }
    errors
}
