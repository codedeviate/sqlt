//! Heuristic statement splitter for the MariaDB raw-fallback path.
//!
//! Used only when `Parser::parse_sql` fails on a multi-statement batch and
//! we need to isolate the offending statement(s) so the rest can still parse
//! into typed AST nodes. We don't aim for SQL-grammar-perfect splitting —
//! the splitter must merely respect string literals, identifier quotes, and
//! line/block comments so that a `;` inside one of those is not mistaken
//! for a statement terminator.

/// Split `sql` on top-level `;` boundaries.
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
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
                    out.push(std::mem::take(&mut buf));
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
        out.push(buf);
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
}
