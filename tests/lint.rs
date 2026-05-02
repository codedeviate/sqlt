//! Per-rule fixture walker.
//!
//! For every directory `tests/fixtures/lint/<RULE_ID>/`, every `<case>.sql`
//! is paired with `<case>.expected.txt` (the text-format rendering). The
//! rule(s) under test are inferred from the directory name. Dialect
//! defaults to `mysql` unless `<case>.dialects` contains a single line
//! `from=<dialect>` or `from=<src>,to=<dst>`.

use std::fs;
use std::path::{Path, PathBuf};

use sqlt::dialect::DialectId;
use sqlt::lint::{self, LintOptions, format};
use sqlt::parse;

fn lint_text(sql: &str, source_label: &str, from: DialectId) -> String {
    let stmts = parse::parse(sql, from).expect("parse");
    let mut diagnostics =
        lint::lint(&stmts, sql, from, None, &LintOptions::default()).expect("lint");
    lint::sort(&mut diagnostics);
    format::render(format::Format::Text, source_label, sql, &diagnostics).expect("render")
}

fn lint_json(sql: &str, source_label: &str, from: DialectId) -> String {
    let stmts = parse::parse(sql, from).expect("parse");
    let mut diagnostics =
        lint::lint(&stmts, sql, from, None, &LintOptions::default()).expect("lint");
    lint::sort(&mut diagnostics);
    format::render(format::Format::Json, source_label, sql, &diagnostics).expect("render")
}

fn lint_sarif(sql: &str, source_label: &str, from: DialectId) -> String {
    let stmts = parse::parse(sql, from).expect("parse");
    let mut diagnostics =
        lint::lint(&stmts, sql, from, None, &LintOptions::default()).expect("lint");
    lint::sort(&mut diagnostics);
    format::render(format::Format::Sarif, source_label, sql, &diagnostics).expect("render")
}

#[test]
fn snapshot_json_select_star() {
    insta::assert_snapshot!(lint_json(
        "SELECT * FROM users",
        "schema.sql",
        DialectId::MySql
    ));
}

#[test]
fn snapshot_sarif_select_star() {
    let sarif = lint_sarif("SELECT * FROM users", "schema.sql", DialectId::MySql);
    // Replace sqlt version + driver version (which embed CARGO_PKG_VERSION) so
    // the snapshot doesn't churn on every minor bump.
    let normalised = sarif.replace(
        &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
        "\"version\": \"<X.Y.Z>\"",
    );
    insta::assert_snapshot!(normalised);
}

#[test]
fn snapshot_text_select_star() {
    insta::assert_snapshot!(lint_text(
        "SELECT * FROM users",
        "schema.sql",
        DialectId::MySql
    ));
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lint")
}

fn parse_dialect_spec(s: &str) -> (DialectId, Option<DialectId>) {
    let mut from = DialectId::MySql;
    let mut to: Option<DialectId> = None;
    for part in s.split(',') {
        let (k, v) = part.split_once('=').unwrap_or(("", part));
        let parsed: DialectId = v.trim().parse().expect("dialect");
        match k.trim() {
            "from" | "" => from = parsed,
            "to" => to = Some(parsed),
            other => panic!("unknown dialect spec key: {other}"),
        }
    }
    (from, to)
}

#[test]
fn lint_fixture_walk() {
    let root = fixtures_dir();
    if !root.exists() {
        return;
    }
    let mut total = 0usize;
    for rule_dir in fs::read_dir(&root).expect("read fixtures") {
        let rule_dir = rule_dir.expect("entry").path();
        if !rule_dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&rule_dir).expect("read rule dir") {
            let path = entry.expect("entry").path();
            let fname = path.file_name().unwrap().to_string_lossy().into_owned();
            let case = match fname.strip_suffix(".sql") {
                Some(c) => c,
                None => continue,
            };
            let expected_path = rule_dir.join(format!("{case}.expected.txt"));
            let expected = fs::read_to_string(&expected_path)
                .unwrap_or_else(|_| panic!("missing {}", expected_path.display()));
            let dialects_path = rule_dir.join(format!("{case}.dialects"));
            let (from, to) = if dialects_path.exists() {
                parse_dialect_spec(&fs::read_to_string(&dialects_path).unwrap())
            } else {
                (DialectId::MySql, None)
            };

            let sql = fs::read_to_string(&path).expect("read sql");
            let stmts = parse::parse(&sql, from)
                .unwrap_or_else(|e| panic!("parse failed for {}: {e}", path.display()));
            let mut diagnostics =
                lint::lint(&stmts, &sql, from, to, &LintOptions::default()).expect("lint");
            lint::sort(&mut diagnostics);

            let actual =
                format::render(format::Format::Text, &fname, &sql, &diagnostics).expect("render");
            assert_eq!(
                actual.trim_end(),
                expected.trim_end(),
                "diagnostic mismatch for {}\n--- expected ---\n{expected}\n--- actual ---\n{actual}",
                path.display()
            );
            total += 1;
        }
    }
    assert!(total > 0, "no lint fixtures found");
}
