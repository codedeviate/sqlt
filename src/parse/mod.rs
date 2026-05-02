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
///
/// MariaDB inputs are first run through [`preprocess_mariadb`] to handle
/// quirks of `mariadb-dump` output that `MySqlDialect` rejects (notably bare
/// `--` comment markers at end-of-line).
pub fn parse(sql: &str, dialect: DialectId) -> Result<Vec<SqltStatement>> {
    let upstream = dialect.upstream();
    let preprocessed = if dialect == DialectId::MariaDb {
        preprocess_mariadb(sql)
    } else {
        sql.to_string()
    };
    match Parser::parse_sql(&*upstream, &preprocessed) {
        Ok(stmts) => Ok(stmts.into_iter().map(SqltStatement::from).collect()),
        Err(e) if dialect == DialectId::MariaDb => {
            mariadb_with_fallback(sql, &preprocessed, &*upstream, e)
        }
        Err(e) => Err(e.into()),
    }
}

/// Pre-process MariaDB SQL to massage out forms that `MySqlDialect` rejects
/// even though the real MariaDB server accepts them.
///
/// Currently handles one case: bare `--` immediately followed by EOL. Real
/// `mariadb-dump` output uses `--` on a line by itself as a separator
/// comment. `MySqlDialect::requires_single_line_comment_whitespace` is `true`,
/// so sqlparser tokenizes that as two minus operators and fails. We turn
/// every `--<EOL>` into `-- <EOL>`. Inside string literals, identifier
/// quotes, and block comments the substitution is suppressed so we don't
/// corrupt data.
fn preprocess_mariadb(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut state = State::Normal;
    while i < bytes.len() {
        let b = bytes[i];
        match state {
            State::Normal => match b {
                b'\'' => {
                    out.push(b as char);
                    state = State::SingleQuote;
                    i += 1;
                }
                b'"' => {
                    out.push(b as char);
                    state = State::DoubleQuote;
                    i += 1;
                }
                b'`' => {
                    out.push(b as char);
                    state = State::Backtick;
                    i += 1;
                }
                b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                    out.push_str("/*");
                    state = State::BlockComment;
                    i += 2;
                }
                b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                    let after = i + 2;
                    let next = bytes.get(after).copied();
                    out.push_str("--");
                    if matches!(next, None | Some(b'\n') | Some(b'\r')) {
                        // Bare `--<EOL>` — inject a space so MySqlDialect
                        // recognises it as a single-line comment.
                        out.push(' ');
                    }
                    state = State::LineComment;
                    i = after;
                }
                _ => {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            },
            State::SingleQuote => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    out.push(bytes[i + 1] as char);
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::DoubleQuote => {
                out.push(bytes[i] as char);
                if bytes[i] == b'"' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::Backtick => {
                out.push(bytes[i] as char);
                if bytes[i] == b'`' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::BlockComment => {
                out.push(bytes[i] as char);
                if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    out.push('/');
                    i += 2;
                    state = State::Normal;
                    continue;
                }
                i += 1;
            }
            State::LineComment => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\n' {
                    state = State::Normal;
                }
                i += 1;
            }
        }
    }
    // Bytes-only iteration assumes the SQL is at most ASCII for the
    // substitution-relevant subset. Multi-byte UTF-8 codepoints pass through
    // each byte unchanged (the patterns we look for are all single-byte
    // ASCII), so the final string is byte-equivalent to the input apart
    // from injected spaces.
    out
}

enum State {
    Normal,
    SingleQuote,
    DoubleQuote,
    Backtick,
    BlockComment,
    LineComment,
}

fn mariadb_with_fallback(
    original_sql: &str,
    preprocessed: &str,
    upstream: &dyn Dialect,
    original_err: sqlparser::parser::ParserError,
) -> Result<Vec<SqltStatement>> {
    // Split the *original* SQL so the byte offsets we report to lint are
    // referring to the user's source file, not the post-preprocessed copy.
    let pieces = split::split_statements_with_lines(original_sql);
    if pieces.is_empty() {
        return Err(original_err.into());
    }
    let _ = preprocessed; // preprocessed is only used by the fast-path call.
    let mut out = Vec::with_capacity(pieces.len());
    let mut any_fallback = false;
    for (start_line, piece) in pieces {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Re-apply preprocessing to each piece so the per-fragment re-parse
        // also accepts bare `--<EOL>`.
        let piece_pp = preprocess_mariadb(trimmed);
        match Parser::parse_sql(upstream, &piece_pp) {
            Ok(mut stmts) if !stmts.is_empty() => {
                out.extend(stmts.drain(..).map(SqltStatement::from));
            }
            _ => {
                let reason = classify_mariadb_raw(trimmed);
                out.push(SqltStatement::Raw(RawStatement {
                    sqlt_raw: trimmed.to_string(),
                    reason,
                    start_line: Some(start_line),
                }));
                any_fallback = true;
            }
        }
    }
    if !any_fallback {
        return Err(original_err.into());
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
    if head.starts_with("DELIMITER") {
        return "delimiter".to_string();
    }
    if head.starts_with("CREATE TRIGGER")
        || head.starts_with("CREATE FUNCTION")
        || head.starts_with("CREATE PROCEDURE")
    {
        return "stored_program_body".to_string();
    }
    "unrepresented".to_string()
}
