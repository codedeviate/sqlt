//! Round-trip correctness suite.
//!
//! For every fixture under `tests/fixtures/<dialect>/*.sql`:
//!   1. parse(sql)            -> stmts1
//!   2. emit(stmts1)          -> sql2
//!   3. parse(sql2)           -> stmts2
//!   4. assert stmts1 == stmts2  (the AST is canonical; surface SQL may differ)
//!   5. JSON round-trip: serialize(stmts1) -> deserialize -> assert == stmts1
//!
//! AST equality is the correctness signal — `Display` infidelities show up
//! as a structural difference between stmts1 and stmts2 and trigger M5 work.

use std::fs;
use std::path::Path;

use sqlt::dialect::DialectId;
use sqlt::emit;
use sqlt::json::{self, Envelope};
use sqlt::parse;

fn fixtures_dir(sub: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(sub)
}

fn check_dialect(dialect: DialectId, sub: &str) {
    let dir = fixtures_dir(sub);
    if !dir.exists() {
        return;
    }
    let mut count = 0usize;
    for entry in fs::read_dir(&dir).expect("read fixtures") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("sql") {
            continue;
        }
        let sql = fs::read_to_string(&path).expect("read fixture");
        let stmts1 = parse::parse(&sql, dialect)
            .unwrap_or_else(|e| panic!("parse failed for {}: {e}", path.display()));

        // SQL round-trip.
        let sql2 = emit::emit(&stmts1, dialect).expect("emit");
        let stmts2 = parse::parse(&sql2, dialect).unwrap_or_else(|e| {
            panic!(
                "re-parse failed for {} (emitted: {sql2:?}): {e}",
                path.display()
            )
        });
        assert_eq!(
            stmts1,
            stmts2,
            "AST mismatch after round-trip for {}\n  original sql: {sql:?}\n  emitted sql:  {sql2:?}",
            path.display()
        );

        // JSON round-trip.
        let env1 = Envelope::new(dialect, stmts1.clone());
        let serialized = json::serialize(&env1, false).expect("serialize");
        let env2 = json::deserialize(&serialized).expect("deserialize");
        assert_eq!(
            env1.statements,
            env2.statements,
            "JSON round-trip mismatch for {}",
            path.display()
        );
        assert_eq!(env1.dialect, env2.dialect);

        count += 1;
    }
    assert!(count > 0, "no fixtures under {}", dir.display());
}

#[test]
fn mysql_roundtrip() {
    check_dialect(DialectId::MySql, "mysql");
}

#[test]
fn mariadb_roundtrip() {
    check_dialect(DialectId::MariaDb, "mariadb");
}

#[test]
fn postgres_roundtrip() {
    check_dialect(DialectId::Postgres, "postgres");
}

#[test]
fn mssql_roundtrip() {
    check_dialect(DialectId::MsSql, "mssql");
}

#[test]
fn sqlite_roundtrip() {
    check_dialect(DialectId::Sqlite, "sqlite");
}
