use super::*;

/// Trimmed `cargo llvm-cov report --json --summary-only` output: two data entries, extra
/// fields that the gate must ignore.
const SUMMARY: &str = r#"{
  "type": "llvm.coverage.json.export",
  "version": "2.0.1",
  "cargo_llvm_cov": { "version": "0.9.1", "manifest_path": "/ws/Cargo.toml" },
  "data": [
    {
      "files": [
        {
          "filename": "/ws/crates/api/src/lib.rs",
          "summary": {
            "lines": { "count": 20, "covered": 18, "percent": 90.0 },
            "functions": { "count": 3, "covered": 3, "percent": 100.0 }
          }
        },
        {
          "filename": "/ws/crates/domain/src/lib.rs",
          "summary": { "lines": { "count": 0, "covered": 0, "percent": 0.0 } }
        }
      ],
      "totals": { "lines": { "count": 20, "covered": 18, "percent": 90.0 } }
    },
    {
      "files": [
        {
          "filename": "/ws/crates/config/src/lib.rs",
          "summary": { "lines": { "count": 10, "covered": 5, "percent": 50.0 } }
        }
      ]
    }
  ]
}"#;

fn config(text: &str) -> Config {
    Config::parse(text).expect("fixture coverage.toml parses")
}

fn package(name: &str, dir: &str) -> Package {
    Package {
        name: name.to_owned(),
        dir: PathBuf::from(dir),
    }
}

fn file(path: &str, covered: u64, count: u64) -> FileLines {
    FileLines {
        path: PathBuf::from(path),
        lines: Lines { count, covered },
    }
}

fn packages() -> Vec<Package> {
    vec![
        package("vak-api", "/ws/crates/api"),
        package("vak-api-extra", "/ws/crates/api-extra"),
        package("vak-domain", "/ws/crates/domain"),
        package("vak-outer", "/ws/crates/outer"),
        package("vak-inner", "/ws/crates/outer/inner"),
    ]
}

fn row<'r>(report: &'r Report, name: &str) -> &'r Row {
    report
        .rows
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row for {name}"))
}

#[test]
fn parses_file_line_counts_from_every_data_entry() {
    let files = parse_summary(SUMMARY).expect("summary parses");

    assert_eq!(
        files,
        vec![
            file("/ws/crates/api/src/lib.rs", 18, 20),
            file("/ws/crates/domain/src/lib.rs", 0, 0),
            file("/ws/crates/config/src/lib.rs", 5, 10),
        ]
    );
}

#[test]
fn rejects_output_that_is_not_an_llvm_cov_export() {
    assert!(parse_summary(r#"{"files": []}"#).is_err());
    assert!(parse_summary("not json").is_err());
}

#[test]
fn crate_above_its_floor_passes() {
    let files = [file("/ws/crates/api/src/lib.rs", 9, 10)];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    assert!(row(&report, "vak-api").passes());
    assert!(report.passed());
}

#[test]
fn crate_below_its_floor_fails_and_the_message_names_it() {
    let files = [file("/ws/crates/api/src/lib.rs", 8, 10)];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    let api = row(&report, "vak-api");
    assert!(!api.passes());
    assert!(!report.passed());
    let message = api.to_string();
    for part in ["vak-api", "80.0%", "85%", "8/10"] {
        assert!(
            message.contains(part),
            "{message:?} should mention {part:?}"
        );
    }
}

#[test]
fn coverage_exactly_at_the_floor_passes() {
    let files = [file("/ws/crates/api/src/lib.rs", 85, 100)];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    assert!(report.passed());
}

#[test]
fn coverage_a_fraction_below_the_floor_fails() {
    let files = [file("/ws/crates/api/src/lib.rs", 849, 1000)];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    assert!(!report.passed());
}

#[test]
fn crates_with_zero_instrumented_lines_pass() {
    let files = [file("/ws/crates/domain/src/lib.rs", 0, 0)];

    let report = gate(
        &config("[crates]\nvak-domain = 95\nvak-inner = 75\n"),
        &packages(),
        &files,
    );

    assert!(row(&report, "vak-domain").passes());
    assert!(
        row(&report, "vak-inner").passes(),
        "a floored crate with no files is listed and passes"
    );
    assert!(report.passed());
}

#[test]
fn files_from_several_sources_add_up_per_crate() {
    let files = [
        file("/ws/crates/api/src/lib.rs", 10, 10),
        file("/ws/crates/api/src/health.rs", 0, 10),
    ];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    assert_eq!(
        row(&report, "vak-api").lines,
        Lines {
            count: 20,
            covered: 10
        }
    );
    assert!(!report.passed());
}

#[test]
fn files_belong_to_the_innermost_package() {
    let files = [
        file("/ws/crates/outer/inner/src/lib.rs", 1, 10),
        file("/ws/crates/outer/src/lib.rs", 10, 10),
    ];

    let report = gate(
        &config("[crates]\nvak-outer = 90\nvak-inner = 5\n"),
        &packages(),
        &files,
    );

    assert_eq!(
        row(&report, "vak-outer").lines,
        Lines {
            count: 10,
            covered: 10
        }
    );
    assert_eq!(
        row(&report, "vak-inner").lines,
        Lines {
            count: 10,
            covered: 1
        }
    );
    assert!(report.passed());
}

#[test]
fn package_prefixes_match_whole_path_components() {
    let files = [file("/ws/crates/api-extra/src/lib.rs", 0, 10)];

    let report = gate(&config("[crates]\nvak-api = 85\n"), &packages(), &files);

    assert_eq!(row(&report, "vak-api").lines, Lines::default());
    assert_eq!(
        row(&report, "vak-api-extra").lines,
        Lines {
            count: 10,
            covered: 0
        }
    );
    assert!(report.passed(), "vak-api-extra has no floor");
}

#[test]
fn crates_without_a_floor_are_listed_but_never_fail() {
    let files = [file("/ws/crates/api/src/lib.rs", 0, 10)];

    let report = gate(&config(""), &packages(), &files);

    let api = row(&report, "vak-api");
    assert_eq!(api.floor, None);
    assert!(api.passes());
    assert!(report.passed());
}

#[test]
fn rows_are_sorted_by_crate_name() {
    let files = [
        file("/ws/crates/outer/src/lib.rs", 1, 1),
        file("/ws/crates/api/src/lib.rs", 1, 1),
    ];

    let report = gate(&config(""), &packages(), &files);

    let names: Vec<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["vak-api", "vak-outer"]);
}

#[test]
fn global_floor_covers_every_file_in_the_summary() {
    let files = [
        file("/ws/crates/api/src/lib.rs", 9, 10),
        file("/elsewhere/generated.rs", 1, 10),
    ];

    let report = gate(&config("global = 80\n"), &packages(), &files);

    let global = report.global.as_ref().expect("global row");
    assert_eq!(
        global.lines,
        Lines {
            count: 20,
            covered: 10
        }
    );
    assert!(!global.passes());
    assert!(!report.passed());
    assert!(global.to_string().contains("50.0%"), "{global}");
}

#[test]
fn global_floor_passes_when_the_total_is_high_enough() {
    let files = [file("/ws/crates/api/src/lib.rs", 8, 10)];

    let report = gate(&config("global = 80\n"), &packages(), &files);

    assert!(report.global.as_ref().is_some_and(Row::passes));
    assert!(report.passed());
}

#[test]
fn no_global_floor_means_no_global_row() {
    let report = gate(&config(""), &packages(), &[]);

    assert!(report.global.is_none());
    assert!(report.passed());
}

#[test]
fn floors_for_unknown_crates_are_errors() {
    let report = gate(&config("[crates]\nvak-gone = 90\n"), &packages(), &[]);

    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert!(report.errors[0].contains("vak-gone"));
    assert!(!report.passed());
}

#[test]
fn floors_above_100_percent_are_errors() {
    let report = gate(
        &config("global = 101\n[crates]\nvak-api = 120\n"),
        &packages(),
        &[],
    );

    assert_eq!(report.errors.len(), 2, "{:?}", report.errors);
    assert!(!report.passed());
}

#[test]
fn config_rejects_unknown_keys_and_fractional_floors() {
    assert!(Config::parse("[floors]\nvak-api = 85\n").is_err());
    assert!(Config::parse("[crates]\nvak-api = 85.5\n").is_err());
}

/// Guards on the repository's own `coverage.toml`: the floors from docs/plan/quality.md.
#[test]
fn repo_coverage_toml_has_the_quality_md_floors() {
    let config = config(include_str!("../../../coverage.toml"));

    assert_eq!(config.global, Some(80));
    let expected = [
        ("vak-domain", 95),
        ("vak-services", 90),
        ("vak-pipeline", 90),
        ("vak-api", 85),
        ("vak-repositories-pg", 85),
        ("vak-adapter-llm-openai", 75),
        ("vak-adapter-stt-serving", 75),
        ("vak-adapter-tts-serving", 75),
        ("vak-adapter-decision-systemone", 75),
    ];
    for (name, floor) in expected {
        assert_eq!(config.crates.get(name), Some(&floor), "{name}");
    }
    assert_eq!(config.crates.len(), expected.len());
}
