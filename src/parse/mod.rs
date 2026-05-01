use sqlparser::dialect::Dialect;
use sqlparser::parser::Parser;

use crate::ast::{RawStatement, SqltStatement};
use crate::dialect::DialectId;
use crate::error::Result;

mod split;

/// Parse SQL into a list of statements.
///
/// For all dialects we first try the fast path: hand the entire input to
/// `sqlparser::Parser::parse_sql`. If that succeeds, every statement is
/// wrapped as `SqltStatement::Std`.
///
/// If parsing fails *and* the dialect is MariaDB, we fall through to the
/// `mariadb_with_fallback` path: split the input on statement boundaries,
/// re-parse each piece individually, and wrap any piece that still fails
/// as `SqltStatement::Raw` with a reason tag classifying the unrepresented
/// construct (system versioning, packages, etc.). For non-MariaDB
/// dialects, the parser error propagates unchanged.
pub fn parse(sql: &str, dialect: DialectId) -> Result<Vec<SqltStatement>> {
    let upstream = dialect.upstream();
    match Parser::parse_sql(&*upstream, sql) {
        Ok(stmts) => Ok(stmts.into_iter().map(SqltStatement::from).collect()),
        Err(e) if dialect == DialectId::MariaDb => mariadb_with_fallback(sql, &*upstream, e),
        Err(e) => Err(e.into()),
    }
}

fn mariadb_with_fallback(
    sql: &str,
    upstream: &dyn Dialect,
    original: sqlparser::parser::ParserError,
) -> Result<Vec<SqltStatement>> {
    let pieces = split::split_statements(sql);
    if pieces.is_empty() {
        return Err(original.into());
    }
    let mut out = Vec::with_capacity(pieces.len());
    let mut any_fallback = false;
    for piece in pieces {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }
        match Parser::parse_sql(upstream, trimmed) {
            Ok(mut stmts) if !stmts.is_empty() => {
                out.extend(stmts.drain(..).map(SqltStatement::from));
            }
            _ => {
                let reason = classify_mariadb_raw(trimmed);
                out.push(SqltStatement::Raw(RawStatement {
                    sqlt_raw: trimmed.to_string(),
                    reason,
                }));
                any_fallback = true;
            }
        }
    }
    if !any_fallback {
        // Splitting accepted everything but the whole-batch parse failed —
        // that's a different kind of bug and we shouldn't hide it.
        return Err(original.into());
    }
    Ok(out)
}

/// Classify a raw MariaDB statement by the first MariaDB-specific token we
/// recognize. Used purely for warning messages.
fn classify_mariadb_raw(stmt: &str) -> String {
    let upper = stmt.to_ascii_uppercase();
    let head = upper.trim_start();
    if head.contains("WITH SYSTEM VERSIONING") || head.contains("PERIOD FOR SYSTEM_TIME") {
        return "system_versioning".to_string();
    }
    if head.contains("FOR SYSTEM_TIME") {
        return "temporal_query".to_string();
    }
    if head.starts_with("CREATE PACKAGE") || head.starts_with("CREATE OR REPLACE PACKAGE") {
        return "create_package".to_string();
    }
    if head.starts_with("CREATE SEQUENCE") || head.starts_with("CREATE OR REPLACE SEQUENCE") {
        return "sequence_options".to_string();
    }
    if head.contains("VECTOR(") || head.contains("VEC_DISTANCE") {
        return "vector_type".to_string();
    }
    "unrepresented".to_string()
}
