//! Heuristic statement splitter for the MariaDB raw-fallback path.
//!
//! Used only when `Parser::parse_sql` fails on a multi-statement batch and
//! we need to isolate the offending statement(s) so the rest can still parse
//! into typed AST nodes. We don't aim for SQL-grammar-perfect splitting —
//! the splitter must merely respect string literals, identifier quotes, and
//! line/block comments so that a `;` inside one of those is not mistaken
//! for a statement terminator.

/// Split `sql` on top-level `;` boundaries. Discards line tracking; use
/// [`split_statements_with_lines`] when you need to attribute each piece to
/// its source location. Used in tests; the parse path uses the with-lines
/// variant directly so source positions survive into raw fragments.
#[cfg(test)]
pub fn split_statements(sql: &str) -> Vec<String> {
    split_statements_with_lines(sql)
        .into_iter()
        .map(|(_, s)| s)
        .collect()
}

/// Split `sql` and tag each piece with the 1-based line of its first
/// character. Lines after a piece-internal newline still belong to that
/// piece — the line number is the *start*.
pub fn split_statements_with_lines(sql: &str) -> Vec<(u64, String)> {
    let mut out: Vec<(u64, String)> = Vec::new();
    let mut buf = String::new();
    // Set on the first non-whitespace character that lands in `buf`.
    let mut buf_start_line: Option<u64> = None;
    let mut current_line: u64 = 1;
    let mut chars = sql.chars().peekable();

    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        Backtick,
        LineComment,
        BlockComment,
    }
    let mut state = State::Normal;

    while let Some(c) = chars.next() {
        // Capture the line of the first non-whitespace character of the
        // current piece — that's what users want to see in diagnostics.
        if buf_start_line.is_none() && !c.is_whitespace() {
            buf_start_line = Some(current_line);
        }
        if c == '\n' {
            current_line += 1;
        }
        match state {
            State::Normal => match c {
                '\'' => {
                    buf.push(c);
                    state = State::SingleQuote;
                }
                '"' => {
                    buf.push(c);
                    state = State::DoubleQuote;
                }
                '`' => {
                    buf.push(c);
                    state = State::Backtick;
                }
                '-' if chars.peek() == Some(&'-') => {
                    buf.push(c);
                    buf.push(chars.next().unwrap());
                    state = State::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    buf.push(c);
                    buf.push(chars.next().unwrap());
                    state = State::BlockComment;
                }
                ';' => {
                    let piece = std::mem::take(&mut buf);
                    out.push((buf_start_line.unwrap_or(current_line), piece));
                    buf_start_line = None;
                }
                _ => buf.push(c),
            },
            State::SingleQuote => {
                buf.push(c);
                if c == '\\' {
                    if let Some(&next) = chars.peek() {
                        buf.push(next);
                        chars.next();
                    }
                } else if c == '\'' {
                    state = State::Normal;
                }
            }
            State::DoubleQuote => {
                buf.push(c);
                if c == '"' {
                    state = State::Normal;
                }
            }
            State::Backtick => {
                buf.push(c);
                if c == '`' {
                    state = State::Normal;
                }
            }
            State::LineComment => {
                buf.push(c);
                if c == '\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                buf.push(c);
                if c == '*' && chars.peek() == Some(&'/') {
                    buf.push(chars.next().unwrap());
                    state = State::Normal;
                }
            }
        }
    }
    if !buf.is_empty() {
        out.push((buf_start_line.unwrap_or(current_line), buf));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple() {
        let v = split_statements("SELECT 1; SELECT 2");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].trim(), "SELECT 1");
        assert_eq!(v[1].trim(), "SELECT 2");
    }

    #[test]
    fn ignores_semicolon_in_string() {
        let v = split_statements("SELECT ';'; SELECT 2");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn ignores_semicolon_in_block_comment() {
        let v = split_statements("/* a; b */ SELECT 1; SELECT 2");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn ignores_semicolon_in_line_comment() {
        let v = split_statements("-- a;\nSELECT 1; SELECT 2");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn ignores_semicolon_in_backticks() {
        let v = split_statements("SELECT `co;l` FROM t; SELECT 2");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn handles_trailing_semicolon() {
        let v = split_statements("SELECT 1;");
        let nonempty: Vec<_> = v.iter().filter(|s| !s.trim().is_empty()).collect();
        assert_eq!(nonempty.len(), 1);
    }

    #[test]
    fn tags_pieces_with_their_starting_line() {
        let v = split_statements_with_lines("SELECT 1;\n\nSELECT 2;\n  -- gap\n\nSELECT 3");
        // Pieces: (line 1, "SELECT 1"), (line 3, "\n\nSELECT 2"), (line 4..., remainder).
        assert!(v.len() >= 2);
        assert_eq!(v[0].0, 1);
        assert!(v[0].1.contains("SELECT 1"));
        assert_eq!(v[1].0, 3);
        assert!(v[1].1.contains("SELECT 2"));
    }
}
