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
/// Two transformations:
///
/// 1. **Bare `--<EOL>`.** Real `mariadb-dump` output uses `--` on a line by
///    itself as a separator. `MySqlDialect::requires_single_line_comment_whitespace`
///    is `true`, so sqlparser tokenizes that as two minus operators. We
///    inject a space after every bare `--` at end-of-line.
///
/// 2. **MySQL/MariaDB conditional comments** (`/*!N …*/`, `/*M!N …*/`).
///    `mariadb-dump` wraps version-gated SQL inside these markers — to the
///    real server they're statements to execute when its version meets the
///    threshold; to a non-MariaDB tokenizer they're opaque block comments.
///    We unwrap them: `/*!40101 SET NAMES latin1 */` becomes
///    `         SET NAMES latin1   `. The marker characters are replaced
///    by spaces of equal length so source line/column positions are
///    preserved end-to-end. We also handle the bare `/*!` (no version
///    digits) and the MariaDB-specific `/*M!` form.
///
/// Inside string literals, identifier quotes, and line comments the
/// substitutions are suppressed so we don't corrupt data.
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
                    // Detect MariaDB/MySQL conditional comments:
                    //   /*!NNN  ...*/    — MySQL conditional comment
                    //   /*M!NNN ...*/    — MariaDB conditional comment
                    //   /*!     ...*/    — unconditional MySQL extension
                    //   /*M!    ...*/    — unconditional MariaDB extension
                    let after = i + 2;
                    let conditional = match bytes.get(after) {
                        Some(b'!') => Some(after + 1),
                        Some(b'M') if bytes.get(after + 1) == Some(&b'!') => Some(after + 2),
                        _ => None,
                    };
                    if let Some(mut cursor) = conditional {
                        // Find the matching `*/`. Conditional comments don't
                        // nest in MariaDB-dump output, so a flat scan is fine.
                        let mut end = None;
                        let mut k = cursor;
                        while k + 1 < bytes.len() {
                            if bytes[k] == b'*' && bytes[k + 1] == b'/' {
                                end = Some(k);
                                break;
                            }
                            k += 1;
                        }
                        if let Some(end) = end {
                            // Replace the `/*!N` (or `/*M!N`) with spaces so
                            // column counts are preserved.
                            for _ in i..cursor {
                                out.push(' ');
                            }
                            // Skip past optional version digits.
                            while cursor < end && bytes[cursor].is_ascii_digit() {
                                out.push(' ');
                                cursor += 1;
                            }
                            // Re-append the inner SQL verbatim (newlines etc
                            // pass through).
                            for &b in &bytes[cursor..end] {
                                out.push(b as char);
                            }
                            // Replace closing `*/` with spaces.
                            out.push(' ');
                            out.push(' ');
                            i = end + 2;
                            continue;
                        }
                        // Unterminated — fall through to normal block-comment
                        // handling so we don't lose data.
                    }
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
        // Pad the input with leading newlines so the parser's tokenizer
        // produces AST `Location`s in the original file's line space.
        // sqlparser counts lines from 1, so to make the first line of the
        // fragment land on `start_line` we need `start_line - 1` newlines.
        // Performance: O(n) string concat, but n is at most the file's line
        // count which is bounded by file size — millions of lines per second.
        let pad_lines = (start_line as usize).saturating_sub(1);
        let padded = if pad_lines == 0 {
            piece_pp
        } else {
            let mut s = String::with_capacity(pad_lines + piece_pp.len());
            for _ in 0..pad_lines {
                s.push('\n');
            }
            s.push_str(&piece_pp);
            s
        };
        match Parser::parse_sql(upstream, &padded) {
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
    let mut head = upper.trim_start();
    // Peek through a leading conditional comment so the classifier sees the
    // *inner* statement: `/*!40000 ALTER TABLE …DISABLE KEYS */` should
    // classify on the ALTER TABLE, not on the comment marker.
    if let Some(rest) = head
        .strip_prefix("/*!")
        .or_else(|| head.strip_prefix("/*M!"))
    {
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        let rest = rest.trim_start();
        head = rest;
    }
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
    if head.starts_with("ALTER TABLE")
        && (head.contains(" DISABLE KEYS") || head.contains(" ENABLE KEYS"))
    {
        return "optimization_hint".to_string();
    }
    if head.starts_with("CREATE DEFINER=")
        || head.starts_with("CREATE ALGORITHM=")
        || head.contains(" DEFINER=`")
    {
        return "definer_clause".to_string();
    }
    if head.starts_with("CREATE EVENT") {
        return "create_event".to_string();
    }
    "unrepresented".to_string()
}
