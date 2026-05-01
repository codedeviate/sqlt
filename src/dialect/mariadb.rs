//! `MariaDbDialect` — a wrapper around `sqlparser`'s `MySqlDialect` that
//! flips the capability flags MariaDB needs (e.g. RETURNING is universally
//! accepted at parse time today, but future upstream gating would require
//! this).
//!
//! MariaDB syntax that has no typed upstream representation
//! (`WITH SYSTEM VERSIONING`, `FOR SYSTEM_TIME`, `CREATE PACKAGE`, MariaDB
//! sequence option ordering, vector types) is *not* handled here at the
//! parser-trait level. The `parse` module recognizes those patterns
//! syntactically and wraps the offending statement as
//! `SqltStatement::Raw(...)`. This keeps `MariaDbDialect` a thin Dialect
//! impl rather than reimplementing parts of the parser.

use sqlparser::dialect::{Dialect, MySqlDialect};

#[derive(Debug)]
pub struct MariaDbDialect {
    inner: MySqlDialect,
}

impl MariaDbDialect {
    pub fn new() -> Self {
        Self {
            inner: MySqlDialect {},
        }
    }
}

impl Default for MariaDbDialect {
    fn default() -> Self {
        Self::new()
    }
}

impl Dialect for MariaDbDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        self.inner.is_identifier_start(ch)
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        self.inner.is_identifier_part(ch)
    }

    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        self.inner.is_delimited_identifier_start(ch)
    }

    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        self.inner.identifier_quote_style(identifier)
    }

    fn supports_string_literal_backslash_escape(&self) -> bool {
        self.inner.supports_string_literal_backslash_escape()
    }

    fn supports_numeric_prefix(&self) -> bool {
        self.inner.supports_numeric_prefix()
    }

    fn supports_named_fn_args_with_eq_operator(&self) -> bool {
        self.inner.supports_named_fn_args_with_eq_operator()
    }
}
