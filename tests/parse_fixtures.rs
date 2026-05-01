//! Smoke test: every fixture under `tests/fixtures/<dialect>/*.sql` must parse
//! cleanly with its declared dialect.

use std::fs;
use std::path::Path;

use sqlt::dialect::DialectId;
use sqlt::parse;

fn fixtures_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|_| Path::new(env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn check_dialect(dialect: DialectId, dir: &str) {
    let root = fixtures_root().join("tests/fixtures").join(dir);
    if !root.exists() {
        return;
    }
    let mut count = 0usize;
    for entry in fs::read_dir(&root).expect("read fixtures dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sql") {
            continue;
        }
        let sql = fs::read_to_string(&path).expect("read fixture");
        let stmts = parse::parse(&sql, dialect)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
        assert!(
            !stmts.is_empty(),
            "fixture {} parsed to empty statement list",
            path.display()
        );
        count += 1;
    }
    assert!(count > 0, "no fixtures found under {}", root.display());
}

#[test]
fn mysql_fixtures_parse() {
    check_dialect(DialectId::MySql, "mysql");
}

#[test]
fn postgres_fixtures_parse() {
    check_dialect(DialectId::Postgres, "postgres");
}

#[test]
fn mssql_fixtures_parse() {
    check_dialect(DialectId::MsSql, "mssql");
}

#[test]
fn sqlite_fixtures_parse() {
    check_dialect(DialectId::Sqlite, "sqlite");
}
